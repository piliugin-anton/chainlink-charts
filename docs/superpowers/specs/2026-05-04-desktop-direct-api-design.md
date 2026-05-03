# Desktop: Direct Chainlink API (no BFF)

**Date:** 2026-05-04
**Status:** Approved

## Goal

Remove the dependency on the Next.js BFF (`http://127.0.0.1:3000`). The Rust desktop app connects directly to two Chainlink APIs:

1. **Candlestick API** (JWT auth) — OHLC history
2. **Data Streams SDK** (HMAC WebSocket) — live prices

## Environment Variables

Only two env vars required:

| Var | Purpose |
|-----|---------|
| `CHAINLINK_USER_ID` | JWT login **and** SDK `api_key` |
| `CHAINLINK_API_KEY` | JWT password **and** SDK `user_secret` |

`CHAINLINK_CHARTS_BASE_URL` is removed entirely.

## Hardcoded URLs

| Use | URL |
|-----|-----|
| Candlestick API (history + JWT auth) | `https://priceapi.dataengine.chain.link` |
| SDK REST | `https://api.dataengine.chain.link` |
| SDK WebSocket | `wss://ws.dataengine.chain.link` |

## Architecture

```
Before:
  app.rs ──► bff.rs ──► http://127.0.0.1:3000 (Next.js BFF) ──► Chainlink

After:
  app.rs ──► candlestick.rs ──► https://priceapi.dataengine.chain.link  (JWT, history)
  app.rs ──► stream.rs      ──► wss://ws.dataengine.chain.link          (SDK WebSocket, live)
```

## Module Changes

### Delete `bff.rs`

Replaced by `auth.rs` + `candlestick.rs`.

---

### New `auth.rs` — JWT token cache

Mirrors `src/lib/chainlink/auth.ts`.

```
struct CachedToken { token: String, expires_at: i64 }
type TokenCache = Arc<Mutex<Option<CachedToken>>>;

async fn get_token(client, cache, user_id, api_key) -> Result<String, AuthError>
```

- Calls `POST https://priceapi.dataengine.chain.link/api/v1/authorize`
- Body: `application/x-www-form-urlencoded` → `login={user_id}&password={api_key}`
- Response: `{ "s": "ok", "d": { "access_token": "...", "expiration": 1234567890 } }`
- Caches token; refreshes when `expires_at - 90s < now`
- Returns `Err` if `s != "ok"` or token is missing

---

### New `candlestick.rs` — history fetcher

Replaces `bff.rs`. Keeps the same `HistoryRowsResponse` struct and `fetch_history` public signature so `app.rs` call sites change minimally.

- `fetch_history(client, cache, user_id, api_key, symbol, resolution, from_sec, to_sec)`
- Calls `auth::get_token` to obtain bearer token
- `GET https://priceapi.dataengine.chain.link/api/v1/history/rows?symbol=…&resolution=…&from=…&to=…`
- `Authorization: Bearer {token}` header
- Returns `HistoryRowsResponse { candles: Vec<Vec<f64>>, error: Option<String> }` (unchanged shape)

---

### Rewrite `stream.rs` — SDK WebSocket

Replace HTTP chunked streaming with `chainlink-data-streams-sdk`.

**Setup:**
```rust
let config = Config::new(user_id, api_key, SDK_REST_URL, SDK_WS_URL).build()?;
let feed_ids: Vec<ID> = ASSET_LIST.iter()
    .map(|a| ID::from_hex_str(a.feed_id).unwrap())
    .collect();
let mut stream = Stream::new(&config, feed_ids).await?;
stream.listen().await?;
```

**Loop:**
```rust
loop {
    tokio::select! {
        result = stream.read() => {
            let response = result?;
            let (_, blob) = decode_full_report(&hex::decode(&response.report.full_report[2..])?)?;
            let report = ReportDataV3::decode(&blob)?;
            // benchmark_price is BigInt (int192, 1e18 scale) — convert to f64 then decode
            let raw_f64 = bigint_to_f64(report.benchmark_price);
            let price = decode_chainlink_price(raw_f64);
            let t = event_time_to_unix_sec(report.valid_from_timestamp as i64);
            let sym = feed_id_to_symbol(&response.report.feed_id);
            // push to prices + tick_queue as before
        }
    }
}
stream.close().await?;
```

**`bigint_to_f64`**: format BigInt as decimal string, parse as f64. This avoids pulling in a bignum dependency.

**Feed ID → symbol mapping**: `ASSET_LIST` now has `feed_id: &'static str`. Reverse-lookup at tick time: iterate `ASSET_LIST` comparing `ID::from_hex_str(a.feed_id)` with `response.report.feed_id`.

**Reconnect**: wrap the entire `Stream::new … close` block in the existing exponential backoff loop (same shape as before). SDK `with_ws_max_reconnect` handles lower-level WS drops; outer loop handles fatal SDK errors.

**`StreamUiStatus`**: unchanged. `Unconfigured` variant now triggers when `Stream::new` fails with an auth/credential error (map SDK auth error → `Unconfigured`).

---

### Update `assets.rs` — add `feed_id`

```rust
pub struct AssetRow {
    pub key: &'static str,
    pub label: &'static str,
    pub api_symbol: &'static str,   // used for Candlestick API history
    pub feed_id: &'static str,      // used for SDK WebSocket streaming
}
```

Feed IDs (mainnet — confirm from Chainlink Data Streams account dashboard):

| Asset | `api_symbol` | `feed_id` (mainnet) |
|-------|-------------|---------------------|
| Bitcoin | BTCUSD | obtain from `https://data.chain.link/streams` |
| Ethereum | ETHUSD | obtain from `https://data.chain.link/streams` |
| Solana | SOLUSD | obtain from `https://data.chain.link/streams` |
| XRP | XRPUSD | obtain from `https://data.chain.link/streams` |

Testnet (Arbitrum Sepolia) IDs for reference/testing:
- BTC/USD: `0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439`
- ETH/USD: `0x000359843a543ee2fe414dc14c7e7920ef10f4372990b79d6361cdc0dd1ba782`

---

### Update `app.rs`

- Remove `base_url: String`
- Add `user_id: String`, `api_key: String`, `token_cache: auth::TokenCache`
- Read `CHAINLINK_USER_ID` + `CHAINLINK_API_KEY` from env at startup (panic with clear message if missing)
- Build SDK `Config` with `user_id` as `api_key` and `api_key` as `user_secret`, hardcoded URLs
- Pass `token_cache`, `user_id`, `api_key` to `schedule_history_fetch`
- Pass SDK `Config` to `stream_loop`

---

### Update `Cargo.toml`

Add:
```toml
chainlink-data-streams-sdk    = { version = "1.2.1", features = ["websocket"] }
chainlink-data-streams-report = "1.2.1"
hex                           = "0.4"
```

Remove (if no longer used after stream rewrite):
- `futures-util`

Keep: `reqwest`, `tokio`, `serde`, `serde_json`, `thiserror`, `eframe`, `egui`, `egui_plot`.

---

### `json_chunks.rs`

Keep — has unit tests; delete in a follow-up if confirmed unused after stream rewrite.

---

### Unchanged

- `chart.rs` — OHLC logic untouched
- `price.rs` — `decode_chainlink_price` (1e18) reused for both history and stream
- `unix_time.rs` — untouched
- `Screen` enum, all chart rendering — untouched
- `StreamUiStatus` enum — untouched

## Data Flow Summary

```
History:
  app.rs → candlestick::fetch_history
         → auth::get_token → POST /api/v1/authorize (JWT)
         → GET /api/v1/history/rows?symbol=BTCUSD&… (Bearer)
         → HistoryRowsResponse { candles: [[t,o,h,l,c,v], …] }
         → chart.rs unchanged

Streaming:
  stream::stream_loop(Config)
    → Stream::new(&config, feed_ids) [SDK WebSocket, HMAC]
    → stream.read() → ReportResponse
    → decode_full_report → ReportDataV3
    → benchmark_price (BigInt, 1e18) → bigint_to_f64 → decode_chainlink_price
    → LastPrice { price, t } pushed to prices + tick_queue
    → chart.rs unchanged
```
