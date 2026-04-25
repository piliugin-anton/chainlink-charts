//! HTTP client for Next.js BFF (`/api/chainlink/*`).

use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

/// Matches `HistoryRowsResponse` in `src/lib/chainlink/candles.ts` (row = `[t, o, h, l, c, vol]`).
#[derive(Debug, Clone, Deserialize)]
pub struct HistoryRowsResponse {
    /// Upstream status field (optional); kept for contract parity with the web client.
    #[serde(default)]
    #[allow(dead_code)]
    pub s: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub candles: Vec<Vec<f64>>,
}

#[derive(Debug, Error)]
pub enum BffError {
    #[error("HTTP {0}: {1}")]
    Http(StatusCode, String),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn history_url(
    base: &str,
    symbol: &str,
    resolution: &str,
    from_sec: i64,
    to_sec: i64,
) -> String {
    let base = base.trim_end_matches('/');
    format!(
        "{base}/api/chainlink/history?symbol={symbol}&resolution={resolution}&from={from_sec}&to={to_sec}"
    )
}

pub async fn fetch_history(
    client: &reqwest::Client,
    base: &str,
    symbol: &str,
    resolution: &str,
    from_sec: i64,
    to_sec: i64,
) -> Result<HistoryRowsResponse, BffError> {
    let url = history_url(base, symbol, resolution, from_sec, to_sec);
    let res = client.get(url).send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        return Err(BffError::Http(status, text.chars().take(500).collect()));
    }
    let body: HistoryRowsResponse = serde_json::from_str(&text)?;
    if let Some(ref e) = body.error {
        if !e.is_empty() {
            return Err(BffError::Http(
                status,
                format!("API error field: {e}"),
            ));
        }
    }
    Ok(body)
}
