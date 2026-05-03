use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SKEW_SECS: i64 = 90;

/// Candlestick API authorize body: `login` = user ID, `password` = API key (see Chainlink docs).
#[derive(Serialize)]
struct AuthorizeForm<'a> {
    login: &'a str,
    password: &'a str,
}

#[derive(Clone)]
pub struct CachedToken {
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

/// Drop a cached JWT (e.g. after history returns 401 so we mint a fresh one).
pub fn invalidate_token_cache(cache: &TokenCache) {
    *cache.lock().unwrap() = None;
}

pub async fn get_token(
    client: &Client,
    cache: &TokenCache,
    price_api_base: &str,
    candlestick_user_id: &str,
    candlestick_api_key: &str,
) -> Result<String, AuthError> {
    let base = price_api_base.trim().trim_end_matches('/');
    let login = candlestick_user_id.trim();
    let password = candlestick_api_key.trim();

    {
        let guard = cache.lock().unwrap();
        match guard.as_ref() {
            Some(cached) => {
                let considered_valid = cached.expires_at - SKEW_SECS > now_secs();
                if considered_valid {
                    return Ok(cached.token.clone());
                }
            }
            None => {}
        }
    }

    let url = format!("{base}/api/v1/authorize");
    let form = AuthorizeForm {
        login,
        password,
    };
    let resp = client.post(url).form(&form).send().await?;

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

    let token = data.access_token.trim().to_string();
    if token.is_empty() {
        return Err(AuthError::Invalid("empty access_token".into()));
    }

    *cache.lock().unwrap() = Some(CachedToken {
        token: token.clone(),
        expires_at: data.expiration,
    });

    Ok(token)
}
