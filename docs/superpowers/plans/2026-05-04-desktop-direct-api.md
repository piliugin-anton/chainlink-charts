# Desktop Direct Chainlink API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the Next.js BFF dependency from the Rust desktop app and connect directly to the Chainlink Candlestick API (JWT, history) and Data Streams SDK (HMAC WebSocket, live prices).

**Architecture:** `auth.rs` handles JWT token caching for the Candlestick REST API. `candlestick.rs` replaces `bff.rs` and fetches history directly from `priceapi.dataengine.chain.link`. `stream.rs` is rewritten to use `chainlink-data-streams-sdk` WebSocket with HMAC auth. Both APIs share the same two env vars: `CHAINLINK_USER_ID` (→ JWT `login` / SDK `api_key`) and `CHAINLINK_API_KEY` (→ JWT `password` / SDK `api_secret`).

**Tech Stack:** `chainlink-data-streams-sdk 1.2.1` (WebSocket feature), `chainlink-data-streams-report 1.2.1`, `hex 0.4`, `num_bigint` (transitive, no direct dep needed), `reqwest`, `tokio`, `egui/eframe`.

---

### Task 1: Update Cargo.toml

**Files:**
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Add SDK crates and hex; remove futures-util**

  Replace the `[dependencies]` block in `crates/desktop/Cargo.toml`:

  ```toml
  [package]
  name = "chainlink-charts-desktop"
  version = "0.1.0"
  edition = "2021"
  description = "Native desktop client — connects directly to Chainlink APIs"

  [dependencies]
  eframe       = { version = "0.34", default-features = true, features = ["default_fonts", "glow"] }
  egui         = "0.34"
  egui_plot    = "0.35"
  serde        = { version = "1", features = ["derive"] }
  serde_json   = "1"
  reqwest      = { version = "0.13", default-features = false, features = ["json", "stream", "rustls"] }
  tokio        = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "io-util"] }
  thiserror    = "2"
  hex          = "0.4"
  chainlink-data-streams-sdk    = { version = "1.2.1", features = ["websocket"] }
  chainlink-data-streams-report = "1.2.1"
  ```

  (`futures-util` is removed — no longer needed after the stream rewrite.)

- [ ] **Step 2: Verify dependency resolution**

  Run from the repo root:
  ```bash
  cargo check -p chainlink-charts-desktop 2>&1 | head -30
  ```
  Expected: warnings about unused imports in the existing `bff.rs` / `stream.rs` are fine; no errors about missing crates.

- [ ] **Step 3: Commit**

  ```bash
  git add crates/desktop/Cargo.toml
  git commit -m "chore(desktop): add data-streams SDK and hex deps, drop futures-util"
  ```

---

### Task 2: Add `feed_id` to `assets.rs`

**Files:**
- Modify: `crates/desktop/src/assets.rs`

The `feed_id` strings are **testnet (Arbitrum Sepolia)** IDs used as defaults. **Replace with mainnet IDs from `https://data.chain.link/streams` before running against mainnet.**

- [ ] **Step 1: Write the failing test**

  Add at the bottom of `crates/desktop/src/assets.rs`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use chainlink_data_streams_report::feed_id::ID;

      #[test]
      fn all_feed_ids_parse_and_are_non_empty() {
          for row in ASSET_LIST {
              assert!(!row.feed_id.is_empty(), "feed_id empty for {}", row.key);
              ID::from_hex_str(row.feed_id)
                  .unwrap_or_else(|e| panic!("invalid feed_id for {}: {e}", row.key));
          }
      }
  }
  ```

- [ ] **Step 2: Run test to confirm it fails (struct field missing)**

  ```bash
  cargo test -p chainlink-charts-desktop assets -- --nocapture 2>&1 | tail -10
  ```
  Expected: compile error — `AssetRow` has no field `feed_id`.

- [ ] **Step 3: Update `AssetRow` struct and populate `feed_id`**

  Replace the entire contents of `crates/desktop/src/assets.rs`:

  ```rust
  //! Static asset list (mirrors `src/lib/chainlink/constants.ts`).
  //!
  //! feed_id values below are Arbitrum Sepolia TESTNET IDs.
  //! Replace with mainnet IDs from https://data.chain.link/streams before production use.

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct AssetRow {
      pub key: &'static str,
      pub label: &'static str,
      pub api_symbol: &'static str, // Candlestick API symbol (history)
      pub feed_id: &'static str,    // Data Streams feed ID hex (websocket)
  }

  pub const ASSET_LIST: &[AssetRow] = &[
      AssetRow {
          key: "BTC",
          label: "Bitcoin",
          api_symbol: "BTCUSD",
          feed_id: "0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439",
      },
      AssetRow {
          key: "ETH",
          label: "Ethereum",
          api_symbol: "ETHUSD",
          feed_id: "0x000359843a543ee2fe414dc14c7e7920ef10f4372990b79d6361cdc0dd1ba782",
      },
      AssetRow {
          key: "SOL",
          label: "Solana",
          api_symbol: "SOLUSD",
          // Replace with mainnet SOL/USD feed ID from https://data.chain.link/streams
          feed_id: "0x0003c74bfa2f66d6c2f6e1f3b37b4b44a1d2c3e5f6a7b8c9d0e1f2a3b4c5d6e7",
      },
      AssetRow {
          key: "XRP",
          label: "XRP",
          api_symbol: "XRPUSD",
          // Replace with mainnet XRP/USD feed ID from https://data.chain.link/streams
          feed_id: "0x0003d85e2b1c3a4f5e6d7c8b9a0e1f2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
      },
  ];
  ```

  **Important:** The SOL and XRP `feed_id` values above are placeholders. The test in step 1 will verify they parse as valid hex IDs; replace them with real IDs from the Chainlink dashboard before use.

- [ ] **Step 4: Run test to verify it passes**

  ```bash
  cargo test -p chainlink-charts-desktop assets -- --nocapture 2>&1 | tail -10
  ```
  Expected: `test assets::tests::all_feed_ids_parse_and_are_non_empty ... ok`

- [ ] **Step 5: Commit**

  ```bash
  git add crates/desktop/src/assets.rs
  git commit -m "feat(desktop): add feed_id to AssetRow for SDK WebSocket stream"
  ```

---

### Task 3: Create `auth.rs` — JWT token cache

**Files:**
- Create: `crates/desktop/src/auth.rs`

Mirrors `src/lib/chainlink/auth.ts`. One shared `TokenCache` Arc is created in `app.rs` and passed to every history fetch.

- [ ] **Step 1: Write the failing test**

  Create `crates/desktop/src/auth.rs` with just the test module:

  ```rust
  use std::sync::{Arc, Mutex};
  use std::time::{SystemTime, UNIX_EPOCH};

  fn unix_now() -> i64 {
      SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .map(|d| d.as_secs() as i64)
          .unwrap_or(0)
  }

  const SKEW_SECS: i64 = 90;

  #[derive(Clone)]
  struct CachedToken {
      token: String,
      expires_at: i64,
  }

  pub type TokenCache = Arc<Mutex<Option<CachedToken>>>;

  pub fn new_token_cache() -> TokenCache {
      Arc::new(Mutex::new(None))
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn fresh_token_is_valid() {
          let cache = new_token_cache();
          let expires_at = unix_now() + 3600;
          *cache.lock().unwrap() = Some(CachedToken {
              token: "tok".into(),
              expires_at,
          });
          let locked = cache.lock().unwrap();
          let cached = locked.as_ref().unwrap();
          assert!(cached.expires_at - SKEW_SECS > unix_now());
      }

      #[test]
      fn expired_token_needs_refresh() {
          let cache = new_token_cache();
          let expires_at = unix_now() + 50; // within SKEW_SECS window
          *cache.lock().unwrap() = Some(CachedToken {
              token: "old".into(),
              expires_at,
          });
          let locked = cache.lock().unwrap();
          let cached = locked.as_ref().unwrap();
          assert!(!(cached.expires_at - SKEW_SECS > unix_now()));
      }
  }
  ```

- [ ] **Step 2: Run test to verify it passes (no network needed)**

  ```bash
  cargo test -p chainlink-charts-desktop auth -- --nocapture 2>&1 | tail -10
  ```
  Expected: `test auth::tests::fresh_token_is_valid ... ok` and `test auth::tests::expired_token_needs_refresh ... ok`

- [ ] **Step 3: Implement `get_token` (network function)**

  Add the following to `crates/desktop/src/auth.rs` (after `new_token_cache`, before the test module):

  ```rust
  use reqwest::Client;
  use serde::Deserialize;
  use thiserror::Error;

  const AUTHORIZE_URL: &str =
      "https://priceapi.dataengine.chain.link/api/v1/authorize";

  #[derive(Debug, Error)]
  pub enum AuthError {
      #[error("network: {0}")]
      Network(#[from] reqwest::Error),
      #[error("authorize HTTP {0}: {1}")]
      Http(u16, String),
      #[error("invalid authorize response: {0}")]
      Invalid(String),
  }

  #[derive(Deserialize)]
  struct AuthorizeResponse {
      s: Option<String>,
      d: Option<AuthorizeData>,
  }

  #[derive(Deserialize)]
  struct AuthorizeData {
      access_token: Option<String>,
      expiration: Option<i64>,
  }

  /// Returns a valid Bearer token, refreshing via the Candlestick API if the cached one is expired.
  pub async fn get_token(
      client: &Client,
      cache: &TokenCache,
      user_id: &str,
      api_key: &str,
  ) -> Result<String, AuthError> {
      let now = unix_now();
      {
          let locked = cache.lock().unwrap();
          if let Some(ref c) = *locked {
              if c.expires_at - SKEW_SECS > now {
                  return Ok(c.token.clone());
              }
          }
      }

      let body = format!("login={}&password={}", user_id, api_key);
      let res = client
          .post(AUTHORIZE_URL)
          .header("Content-Type", "application/x-www-form-urlencoded")
          .body(body)
          .send()
          .await?;

      let status = res.status().as_u16();
      let text = res.text().await?;

      if status != 200 {
          return Err(AuthError::Http(
              status,
              text.chars().take(200).collect(),
          ));
      }

      let parsed: AuthorizeResponse = serde_json::from_str(&text)
          .map_err(|e| AuthError::Invalid(e.to_string()))?;

      if parsed.s.as_deref() != Some("ok") {
          return Err(AuthError::Invalid(format!("s={:?}", parsed.s)));
      }

      let d = parsed
          .d
          .ok_or_else(|| AuthError::Invalid("missing d".into()))?;
      let token = d
          .access_token
          .ok_or_else(|| AuthError::Invalid("missing access_token".into()))?;
      let expires_at = match d.expiration {
          Some(exp) if exp > now => exp,
          _ => now + 3600,
      };

      *cache.lock().unwrap() = Some(CachedToken {
          token: token.clone(),
          expires_at,
      });
      Ok(token)
  }
  ```

- [ ] **Step 4: Run tests again to confirm nothing broke**

  ```bash
  cargo test -p chainlink-charts-desktop auth -- --nocapture 2>&1 | tail -10
  ```
  Expected: same two tests still pass.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/desktop/src/auth.rs
  git commit -m "feat(desktop): add auth.rs — JWT token cache for Candlestick API"
  ```

---

### Task 4: Create `candlestick.rs` — history fetcher

**Files:**
- Create: `crates/desktop/src/candlestick.rs`

Replaces `bff.rs`. Keeps the same `HistoryRowsResponse` struct shape and same `fetch_history` call signature so `app.rs` changes are minimal.

- [ ] **Step 1: Write the failing test**

  Create `crates/desktop/src/candlestick.rs` with just the test:

  ```rust
  const HISTORY_BASE: &str = "https://priceapi.dataengine.chain.link";

  fn history_url(symbol: &str, resolution: &str, from_sec: i64, to_sec: i64) -> String {
      format!(
          "{}/api/v1/history/rows?symbol={}&resolution={}&from={}&to={}",
          HISTORY_BASE, symbol, resolution, from_sec, to_sec
      )
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn url_contains_all_params() {
          let url = history_url("BTCUSD", "5m", 1_000_000, 2_000_000);
          assert!(url.starts_with("https://priceapi.dataengine.chain.link"));
          assert!(url.contains("symbol=BTCUSD"));
          assert!(url.contains("resolution=5m"));
          assert!(url.contains("from=1000000"));
          assert!(url.contains("to=2000000"));
      }
  }
  ```

- [ ] **Step 2: Run test to verify it passes**

  ```bash
  cargo test -p chainlink-charts-desktop candlestick -- --nocapture 2>&1 | tail -10
  ```
  Expected: `test candlestick::tests::url_contains_all_params ... ok`

- [ ] **Step 3: Implement the full module**

  Replace the contents of `crates/desktop/src/candlestick.rs`:

  ```rust
  //! Direct Candlestick API client (replaces bff.rs).
  //! History endpoint: GET https://priceapi.dataengine.chain.link/api/v1/history/rows

  use reqwest::Client;
  use serde::Deserialize;
  use thiserror::Error;

  use crate::auth::{self, AuthError, TokenCache};

  const HISTORY_BASE: &str = "https://priceapi.dataengine.chain.link";

  /// Matches the `/api/v1/history/rows` response shape (row = `[t, o, h, l, c, vol]`).
  #[derive(Debug, Clone, Deserialize)]
  pub struct HistoryRowsResponse {
      #[serde(default)]
      #[allow(dead_code)]
      pub s: Option<String>,
      #[serde(default)]
      pub error: Option<String>,
      #[serde(default)]
      pub candles: Vec<Vec<f64>>,
  }

  #[derive(Debug, Error)]
  pub enum CandlestickError {
      #[error("auth: {0}")]
      Auth(#[from] AuthError),
      #[error("HTTP {0}: {1}")]
      Http(u16, String),
      #[error("network: {0}")]
      Network(#[from] reqwest::Error),
      #[error("invalid JSON: {0}")]
      Json(#[from] serde_json::Error),
  }

  fn history_url(symbol: &str, resolution: &str, from_sec: i64, to_sec: i64) -> String {
      format!(
          "{}/api/v1/history/rows?symbol={}&resolution={}&from={}&to={}",
          HISTORY_BASE, symbol, resolution, from_sec, to_sec
      )
  }

  pub async fn fetch_history(
      client: &Client,
      cache: &TokenCache,
      user_id: &str,
      api_key: &str,
      symbol: &str,
      resolution: &str,
      from_sec: i64,
      to_sec: i64,
  ) -> Result<HistoryRowsResponse, CandlestickError> {
      let token = auth::get_token(client, cache, user_id, api_key).await?;
      let url = history_url(symbol, resolution, from_sec, to_sec);
      let res = client
          .get(&url)
          .header("Authorization", format!("Bearer {}", token))
          .send()
          .await?;
      let status = res.status().as_u16();
      let text = res.text().await?;
      if !(200..300).contains(&(status as u32)) {
          return Err(CandlestickError::Http(
              status,
              text.chars().take(500).collect(),
          ));
      }
      let body: HistoryRowsResponse = serde_json::from_str(&text)?;
      if let Some(ref e) = body.error {
          if !e.is_empty() {
              return Err(CandlestickError::Http(status, format!("API error: {e}")));
          }
      }
      Ok(body)
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn url_contains_all_params() {
          let url = history_url("BTCUSD", "5m", 1_000_000, 2_000_000);
          assert!(url.starts_with("https://priceapi.dataengine.chain.link"));
          assert!(url.contains("symbol=BTCUSD"));
          assert!(url.contains("resolution=5m"));
          assert!(url.contains("from=1000000"));
          assert!(url.contains("to=2000000"));
      }
  }
  ```

- [ ] **Step 4: Run tests**

  ```bash
  cargo test -p chainlink-charts-desktop candlestick -- --nocapture 2>&1 | tail -10
  ```
  Expected: `test candlestick::tests::url_contains_all_params ... ok`

- [ ] **Step 5: Commit**

  ```bash
  git add crates/desktop/src/candlestick.rs
  git commit -m "feat(desktop): add candlestick.rs — direct JWT history fetcher"
  ```

---

### Task 5: Rewrite `stream.rs` — SDK WebSocket

**Files:**
- Modify: `crates/desktop/src/stream.rs`

The `LastPrice` and `StreamUiStatus` types stay identical (same fields, same variants). The transport changes from HTTP chunked streaming to SDK WebSocket. `feed_id_to_symbol` maps an `ID` back to a Candlestick API symbol string using `ASSET_LIST`.

- [ ] **Step 1: Write the failing test**

  Add at the bottom of the existing `crates/desktop/src/stream.rs`:

  ```rust
  #[cfg(test)]
  mod tests {
      use chainlink_data_streams_report::feed_id::ID;

      fn feed_id_to_symbol(id: &ID) -> Option<String> {
          let hex = id.to_hex_string();
          crate::assets::ASSET_LIST
              .iter()
              .find(|a| a.feed_id.eq_ignore_ascii_case(&hex))
              .map(|a| a.api_symbol.to_string())
      }

      #[test]
      fn btc_feed_id_resolves_to_btcusd() {
          // Testnet BTC/USD feed ID — same value used in assets.rs
          let id = ID::from_hex_str(
              "0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439",
          )
          .unwrap();
          assert_eq!(feed_id_to_symbol(&id), Some("BTCUSD".to_string()));
      }

      #[test]
      fn unknown_feed_id_returns_none() {
          let id = ID::from_hex_str(
              "0x0000000000000000000000000000000000000000000000000000000000000000",
          )
          .unwrap();
          assert_eq!(feed_id_to_symbol(&id), None);
      }
  }
  ```

- [ ] **Step 2: Run test to confirm it fails (function not in scope)**

  ```bash
  cargo test -p chainlink-charts-desktop stream -- --nocapture 2>&1 | tail -15
  ```
  Expected: compile error referencing `feed_id_to_symbol` not found, or the `use chainlink_data_streams_report` not resolved.

- [ ] **Step 3: Replace `stream.rs` entirely**

  Overwrite `crates/desktop/src/stream.rs` with:

  ```rust
  //! WebSocket live price stream via the Chainlink Data Streams SDK.

  use std::collections::HashMap;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use chainlink_data_streams_report::feed_id::ID;
  use chainlink_data_streams_report::report::decode_full_report;
  use chainlink_data_streams_report::report::v3::ReportDataV3;
  use chainlink_data_streams_sdk::config::Config;
  use chainlink_data_streams_sdk::stream::{Stream, StreamError};
  use tokio::time::sleep;

  use crate::assets::ASSET_LIST;
  use crate::price::decode_chainlink_price;
  use crate::unix_time::event_time_to_unix_sec;

  const SDK_REST_URL: &str = "https://api.dataengine.chain.link";
  const SDK_WS_URL: &str = "wss://ws.dataengine.chain.link";

  /// Maximum ticks buffered per symbol while the UI is not rendering.
  const MAX_PENDING_TICKS: usize = 1000;

  #[derive(Clone, Default)]
  pub struct LastPrice {
      pub price: f64,
      pub t: i64,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum StreamUiStatus {
      Connecting,
      Live,
      Reconnecting,
      Error,
      /// SDK auth failed (bad credentials) — do not reconnect.
      Unconfigured,
  }

  fn feed_id_to_symbol(id: &ID) -> Option<String> {
      let hex = id.to_hex_string(); // lowercase "0x…"
      ASSET_LIST
          .iter()
          .find(|a| a.feed_id.eq_ignore_ascii_case(&hex))
          .map(|a| a.api_symbol.to_string())
  }

  pub async fn stream_loop(
      user_id: String,
      api_key: String,
      ctx: egui::Context,
      prices: Arc<Mutex<HashMap<String, LastPrice>>>,
      tick_queue: Arc<Mutex<HashMap<String, Vec<LastPrice>>>>,
      status: Arc<Mutex<StreamUiStatus>>,
      last_err: Arc<Mutex<Option<String>>>,
  ) {
      let feed_ids: Vec<ID> = ASSET_LIST
          .iter()
          .filter_map(|a| ID::from_hex_str(a.feed_id).ok())
          .collect();

      let config = match Config::new(
          user_id,
          api_key,
          SDK_REST_URL.to_string(),
          SDK_WS_URL.to_string(),
      )
      .build()
      {
          Ok(c) => c,
          Err(e) => {
              *status.lock().unwrap() = StreamUiStatus::Unconfigured;
              *last_err.lock().unwrap() = Some(format!("SDK config error: {e}"));
              ctx.request_repaint();
              return;
          }
      };

      let mut backoff = Duration::from_secs(1);

      loop {
          {
              let mut s = status.lock().unwrap();
              *s = if *s == StreamUiStatus::Live {
                  StreamUiStatus::Reconnecting
              } else {
                  StreamUiStatus::Connecting
              };
          }
          *last_err.lock().unwrap() = None;
          ctx.request_repaint();

          let mut stream = match Stream::new(&config, feed_ids.clone()).await {
              Ok(s) => s,
              Err(StreamError::AuthError(e)) => {
                  *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                  *last_err.lock().unwrap() = Some(format!("Auth error: {e}"));
                  ctx.request_repaint();
                  return;
              }
              Err(e) => {
                  *status.lock().unwrap() = StreamUiStatus::Error;
                  *last_err.lock().unwrap() = Some(e.to_string());
                  ctx.request_repaint();
                  sleep(backoff).await;
                  backoff = (backoff * 2).min(Duration::from_secs(30));
                  continue;
              }
          };

          if let Err(e) = stream.listen().await {
              *status.lock().unwrap() = StreamUiStatus::Error;
              *last_err.lock().unwrap() = Some(e.to_string());
              ctx.request_repaint();
              let _ = stream.close().await;
              sleep(backoff).await;
              backoff = (backoff * 2).min(Duration::from_secs(30));
              continue;
          }

          *status.lock().unwrap() = StreamUiStatus::Live;
          backoff = Duration::from_secs(1);
          ctx.request_repaint();

          loop {
              let ws_report = match stream.read().await {
                  Ok(r) => r,
                  Err(e) => {
                      *status.lock().unwrap() = StreamUiStatus::Error;
                      *last_err.lock().unwrap() = Some(e.to_string());
                      ctx.request_repaint();
                      break;
                  }
              };

              let report = &ws_report.report;

              // full_report is hex-encoded (may have "0x" prefix from some responses)
              let hex_str = report.full_report.trim_start_matches("0x");
              let full_bytes = match hex::decode(hex_str) {
                  Ok(b) => b,
                  Err(_) => continue,
              };

              let (_, blob) = match decode_full_report(&full_bytes) {
                  Ok(r) => r,
                  Err(_) => continue,
              };

              let report_data = match ReportDataV3::decode(&blob) {
                  Ok(r) => r,
                  Err(_) => continue,
              };

              let sym = match feed_id_to_symbol(&report.feed_id) {
                  Some(s) => s,
                  None => continue,
              };

              // benchmark_price is num_bigint::BigInt with 1e18 scale
              let raw: f64 = report_data.benchmark_price.to_string().parse().unwrap_or(0.0);
              let price = decode_chainlink_price(raw);
              let t = event_time_to_unix_sec(report.valid_from_timestamp as i64);
              let lp = LastPrice { price, t };

              {
                  let mut map = prices.lock().unwrap();
                  map.insert(sym.clone(), lp.clone());
              }
              {
                  let mut queue = tick_queue.lock().unwrap();
                  let entry = queue.entry(sym).or_insert_with(Vec::new);
                  entry.push(lp);
                  if entry.len() > MAX_PENDING_TICKS {
                      let excess = entry.len() - MAX_PENDING_TICKS;
                      entry.drain(..excess);
                  }
              }
              ctx.request_repaint();
          }

          let _ = stream.close().await;
          *status.lock().unwrap() = StreamUiStatus::Reconnecting;
          ctx.request_repaint();
          sleep(backoff).await;
          backoff = (backoff * 2).min(Duration::from_secs(30));
      }
  }

  #[cfg(test)]
  mod tests {
      use chainlink_data_streams_report::feed_id::ID;

      use super::feed_id_to_symbol;

      #[test]
      fn btc_feed_id_resolves_to_btcusd() {
          let id = ID::from_hex_str(
              "0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439",
          )
          .unwrap();
          assert_eq!(feed_id_to_symbol(&id), Some("BTCUSD".to_string()));
      }

      #[test]
      fn unknown_feed_id_returns_none() {
          let id = ID::from_hex_str(
              "0x0000000000000000000000000000000000000000000000000000000000000000",
          )
          .unwrap();
          assert_eq!(feed_id_to_symbol(&id), None);
      }
  }
  ```

- [ ] **Step 4: Run tests**

  ```bash
  cargo test -p chainlink-charts-desktop stream -- --nocapture 2>&1 | tail -10
  ```
  Expected: `test stream::tests::btc_feed_id_resolves_to_btcusd ... ok` and `test stream::tests::unknown_feed_id_returns_none ... ok`

- [ ] **Step 5: Commit**

  ```bash
  git add crates/desktop/src/stream.rs
  git commit -m "feat(desktop): rewrite stream.rs using Data Streams SDK WebSocket"
  ```

---

### Task 6: Update `app.rs`

**Files:**
- Modify: `crates/desktop/src/app.rs`

Remove `base_url`, add `user_id`/`api_key`/`token_cache`. Update `schedule_history_fetch` to call `candlestick::fetch_history`. Update `stream_loop` call (no more stream client). Fix status labels.

- [ ] **Step 1: Update imports at top of `app.rs`**

  Replace the import block (lines 1–23) with:

  ```rust
  use std::collections::HashMap;
  use std::ops::RangeInclusive;
  use std::sync::{Arc, Mutex};
  use std::time::{Duration, SystemTime, UNIX_EPOCH};

  use egui::epaint::CornerRadiusF32;
  use egui::{
      emath::Rangef, pos2, vec2, Color32, CursorIcon, FontId, LayerId, Order, PointerButton, Rect,
      Shape, Stroke, Vec2b,
  };
  use egui_plot::{
      uniform_grid_spacer, CoordinatesFormatter, Corner, GridInput, GridMark, HLine, LineStyle, Plot,
      PlotBounds, PlotPoint, PlotResponse, PlotUi,
  };
  use reqwest::Client;

  use crate::assets::ASSET_LIST;
  use crate::auth;
  use crate::candlestick::{self, HistoryRowsResponse};
  use crate::chart::{self, FormingBarState, SealedCandleRow};
  use crate::stream::{self, LastPrice, StreamUiStatus};
  use crate::unix_time;
  use tokio::runtime::Handle;
  ```

- [ ] **Step 2: Update `ChainlinkApp` struct**

  Replace the struct definition (currently lines ~226–236):

  ```rust
  pub struct ChainlinkApp {
      user_id: String,
      api_key: String,
      token_cache: auth::TokenCache,
      runtime: Handle,
      client: Client,
      egui_ctx: egui::Context,
      stream_prices: Arc<Mutex<HashMap<String, LastPrice>>>,
      tick_queue: TickQueue,
      stream_status: Arc<Mutex<StreamUiStatus>>,
      stream_err: Arc<Mutex<Option<String>>>,
      screen: Screen,
  }
  ```

- [ ] **Step 3: Update `ChainlinkApp::new`**

  Replace the entire `new` function body:

  ```rust
  pub fn new(cc: &eframe::CreationContext<'_>, runtime: Handle) -> Self {
      let user_id = std::env::var("CHAINLINK_USER_ID")
          .expect("CHAINLINK_USER_ID env var must be set");
      let api_key = std::env::var("CHAINLINK_API_KEY")
          .expect("CHAINLINK_API_KEY env var must be set");
      let token_cache = auth::new_token_cache();

      let client = Client::builder()
          .timeout(Duration::from_secs(60))
          .build()
          .expect("reqwest client");

      let egui_ctx = cc.egui_ctx.clone();
      let stream_prices = Arc::new(Mutex::new(HashMap::new()));
      let tick_queue: TickQueue = Arc::new(Mutex::new(HashMap::new()));
      let stream_status = Arc::new(Mutex::new(StreamUiStatus::Connecting));
      let stream_err = Arc::new(Mutex::new(None));

      runtime.spawn(stream::stream_loop(
          user_id.clone(),
          api_key.clone(),
          egui_ctx.clone(),
          stream_prices.clone(),
          tick_queue.clone(),
          stream_status.clone(),
          stream_err.clone(),
      ));

      Self {
          user_id,
          api_key,
          token_cache,
          runtime,
          client,
          egui_ctx,
          stream_prices,
          tick_queue,
          stream_status,
          stream_err,
          screen: Screen::List,
      }
  }
  ```

- [ ] **Step 4: Update `schedule_history_fetch`**

  Replace the entire method:

  ```rust
  fn schedule_history_fetch(
      &self,
      api_symbol: String,
      resolution: Resolution,
      slot: HistorySlot,
  ) {
      *slot.lock().expect("history lock") = None;
      let client = self.client.clone();
      let cache = self.token_cache.clone();
      let user_id = self.user_id.clone();
      let api_key = self.api_key.clone();
      let ctx = self.egui_ctx.clone();
      self.runtime.spawn(async move {
          let now = SystemTime::now()
              .duration_since(UNIX_EPOCH)
              .map(|d| d.as_secs() as i64)
              .unwrap_or(0);
          let from = now - 86400;
          let body = candlestick::fetch_history(
              &client,
              &cache,
              &user_id,
              &api_key,
              &api_symbol,
              resolution.as_str(),
              from,
              now,
          )
          .await
          .map_err(|e| e.to_string());
          *slot.lock().expect("history slot") = Some(body);
          ctx.request_repaint();
      });
  }
  ```

- [ ] **Step 5: Update `stream_status_label` and `ui_list` help text**

  Replace the `stream_status_label` method:

  ```rust
  fn stream_status_label(status: StreamUiStatus) -> &'static str {
      match status {
          StreamUiStatus::Connecting => "Stream: connecting…",
          StreamUiStatus::Live => "Stream: live",
          StreamUiStatus::Reconnecting => "Stream: reconnecting…",
          StreamUiStatus::Error => "Stream: error",
          StreamUiStatus::Unconfigured => "Stream: credentials missing or invalid",
      }
  }
  ```

  In `ui_list`, replace the Unconfigured help text (currently `"Set Chainlink environment variables in Next.js and restart the BFF."`):

  ```rust
  if st == StreamUiStatus::Unconfigured {
      ui.label("Set CHAINLINK_USER_ID and CHAINLINK_API_KEY environment variables.");
  }
  ```

- [ ] **Step 6: Compile check**

  ```bash
  cargo check -p chainlink-charts-desktop 2>&1 | grep -E "^error" | head -20
  ```
  Expected: no `error` lines (warnings are fine).

- [ ] **Step 7: Commit**

  ```bash
  git add crates/desktop/src/app.rs
  git commit -m "feat(desktop): wire app.rs to auth/candlestick/stream; remove BFF base_url"
  ```

---

### Task 7: Update `main.rs`, delete `bff.rs`, run full test suite

**Files:**
- Modify: `crates/desktop/src/main.rs`
- Delete: `crates/desktop/src/bff.rs`

- [ ] **Step 1: Update mod declarations in `main.rs`**

  Replace the existing mod list:

  ```rust
  //! Desktop client — connects directly to Chainlink Candlestick API and Data Streams WebSocket.

  mod app;
  mod assets;
  mod auth;
  mod candlestick;
  mod chart;
  mod json_chunks;
  mod price;
  mod stream;
  mod unix_time;

  fn main() -> eframe::Result<()> {
      let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
      let _guard = rt.enter();
      let handle = rt.handle().clone();

      let native_options = eframe::NativeOptions {
          viewport: egui::ViewportBuilder::default()
              .with_inner_size([720.0, 560.0])
              .with_title("Chainlink Charts"),
          ..Default::default()
      };

      eframe::run_native(
          "Chainlink Charts",
          native_options,
          Box::new(move |cc| Ok(Box::new(app::ChainlinkApp::new(cc, handle)))),
      )
  }
  ```

- [ ] **Step 2: Delete `bff.rs`**

  ```bash
  rm crates/desktop/src/bff.rs
  ```

- [ ] **Step 3: Run all tests**

  ```bash
  cargo test -p chainlink-charts-desktop 2>&1 | tail -20
  ```
  Expected output (all pass):
  ```
  test assets::tests::all_feed_ids_parse_and_are_non_empty ... ok
  test auth::tests::expired_token_needs_refresh ... ok
  test auth::tests::fresh_token_is_valid ... ok
  test candlestick::tests::url_contains_all_params ... ok
  test json_chunks::tests::two_objects ... ok
  test json_chunks::tests::split_chunk ... ok
  test stream::tests::btc_feed_id_resolves_to_btcusd ... ok
  test stream::tests::unknown_feed_id_returns_none ... ok
  test unix_time::tests::epoch_utc ... ok
  test unix_time::tests::one_day ... ok
  test unix_time::tests::ms_timestamps_map_to_sec ... ok
  test result: ok. 11 passed; 0 failed
  ```

- [ ] **Step 4: Build release binary**

  ```bash
  cargo build --release -p chainlink-charts-desktop 2>&1 | tail -5
  ```
  Expected: `Finished release [optimized] target(s) in ...`

- [ ] **Step 5: Smoke test with env vars**

  Set your credentials and run:
  ```bash
  CHAINLINK_USER_ID=your_user_id CHAINLINK_API_KEY=your_api_key \
    cargo run --release -p chainlink-charts-desktop
  ```
  Expected behaviour:
  - Window opens; status shows "Stream: connecting…" then "Stream: live"
  - Asset list is populated; clicking an asset opens a chart with candles loading
  - Live price overlay appears once stream ticks arrive
  - **If SOL/XRP feed IDs are still placeholder values**, the stream will silently drop reports for those assets — BTC and ETH should still work

- [ ] **Step 6: Final commit**

  ```bash
  git add crates/desktop/src/main.rs
  git commit -m "feat(desktop): remove bff.rs; desktop now connects directly to Chainlink APIs"
  ```
