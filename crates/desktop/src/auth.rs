use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

const AUTHORIZE_URL: &str = "https://priceapi.dataengine.chain.link/api/v1/authorize";
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
    s: String,
    d: Option<AuthorizeData>,
    errmsg: Option<String>,
}

#[derive(Deserialize)]
struct AuthorizeData {
    access_token: String,
    expiration: i64,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub async fn get_token(
    client: &Client,
    cache: &TokenCache,
    user_id: &str,
    api_key: &str,
) -> Result<String, AuthError> {
    {
        let guard = cache.lock().unwrap();
        if let Some(ref cached) = *guard {
            if cached.expires_at - SKEW_SECS > now_secs() {
                return Ok(cached.token.clone());
            }
        }
    }

    let resp = client
        .post(AUTHORIZE_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("login={}&password={}", user_id, api_key))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AuthError::Http(
            status.as_u16(),
            body.chars().take(200).collect(),
        ));
    }

    let parsed: AuthorizeResponse = resp.json().await?;

    if parsed.s != "ok" {
        return Err(AuthError::Invalid(
            parsed.errmsg.unwrap_or_else(|| format!("s={}", parsed.s)),
        ));
    }

    let data = parsed.d.ok_or_else(|| AuthError::Invalid("missing d field".into()))?;

    let token = data.access_token.clone();
    *cache.lock().unwrap() = Some(CachedToken {
        token: data.access_token,
        expires_at: data.expiration,
    });

    Ok(token)
}
