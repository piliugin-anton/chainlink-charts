use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

use crate::auth::{self, AuthError, TokenCache};

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
    #[error("history API: {0}")]
    Api(String),
}

pub async fn fetch_history(
    client: &Client,
    cache: &TokenCache,
    price_api_base: &str,
    candlestick_user_id: &str,
    candlestick_api_key: &str,
    symbol: &str,
    resolution: &str,
    from_sec: i64,
    to_sec: i64,
) -> Result<HistoryRowsResponse, CandlestickError> {
    let base = price_api_base.trim().trim_end_matches('/');
    let from_s = from_sec.to_string();
    let to_s = to_sec.to_string();

    for attempt in 0u8..2 {
        let token = auth::get_token(client, cache, base, candlestick_user_id, candlestick_api_key)
            .await?;

        let resp = client
            .get(format!("{base}/api/v1/history/rows"))
            .query(&[
                ("symbol", symbol),
                ("resolution", resolution),
                ("from", from_s.as_str()),
                ("to", to_s.as_str()),
            ])
            .bearer_auth(token.trim())
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
            auth::invalidate_token_cache(cache);
            continue;
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CandlestickError::Http(
                status.as_u16(),
                body.chars().take(200).collect(),
            ));
        }

        let data: HistoryRowsResponse = resp.json().await?;
        return finish_history_response(data);
    }

    Err(CandlestickError::Api(
        "history: repeated 401 after token refresh".into(),
    ))
}

fn finish_history_response(data: HistoryRowsResponse) -> Result<HistoryRowsResponse, CandlestickError> {
    if let Some(msg) = &data.error {
        return Err(CandlestickError::Api(msg.clone()));
    }
    if data.s.as_deref() == Some("error") {
        return Err(CandlestickError::Api(
            "history response status is error".into(),
        ));
    }
    Ok(data)
}
