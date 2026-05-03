use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

use crate::auth::{self, AuthError, TokenCache};

const HISTORY_BASE: &str = "https://priceapi.dataengine.chain.link";

#[derive(Debug, Deserialize, Default)]
pub struct HistoryRowsResponse {
    pub s: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub candles: Vec<Vec<f64>>,
}

#[derive(Debug, Error)]
pub enum CandlestickError {
    #[error("auth: {0}")]
    Auth(#[from] AuthError),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("history HTTP {0}: {1}")]
    Http(u16, String),
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

    let url = format!(
        "{}/api/v1/history/rows?symbol={}&resolution={}&from={}&to={}",
        HISTORY_BASE, symbol, resolution, from_sec, to_sec
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(CandlestickError::Http(
            status.as_u16(),
            body.chars().take(200).collect(),
        ));
    }

    let data: HistoryRowsResponse = resp.json().await?;
    Ok(data)
}
