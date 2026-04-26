//! Background worker for `GET /api/chainlink/stream` (brace-balanced JSON chunks).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::time::sleep;

use crate::json_chunks::feed_json_chunks;
use crate::price::decode_chainlink_price;
use crate::unix_time::event_time_to_unix_sec;

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
    Unconfigured,
}

fn price_field_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().or_else(|| n.as_i64().map(|i| i as f64)),
        _ => None,
    }
}

fn time_field_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        _ => None,
    }
}

pub async fn stream_loop(
    base_url: String,
    client: reqwest::Client,
    ctx: egui::Context,
    prices: Arc<Mutex<HashMap<String, LastPrice>>>,
    status: Arc<Mutex<StreamUiStatus>>,
    last_err: Arc<Mutex<Option<String>>>,
) {
    let url = format!("{}/api/chainlink/stream", base_url.trim_end_matches('/'));
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

        let res = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                *status.lock().unwrap() = StreamUiStatus::Error;
                *last_err.lock().unwrap() = Some(e.to_string());
                ctx.request_repaint();
                sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
                continue;
            }
        };

        if res.status() == StatusCode::SERVICE_UNAVAILABLE {
            *status.lock().unwrap() = StreamUiStatus::Unconfigured;
            ctx.request_repaint();
            return;
        }

        if !res.status().is_success() {
            let status_code = res.status();
            let t = res.text().await.unwrap_or_default();
            *status.lock().unwrap() = StreamUiStatus::Error;
            *last_err.lock().unwrap() = Some(format!(
                "HTTP {} {}",
                status_code,
                t.chars().take(200).collect::<String>()
            ));
            ctx.request_repaint();
            sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
            continue;
        }

        *status.lock().unwrap() = StreamUiStatus::Live;
        backoff = Duration::from_secs(1);
        ctx.request_repaint();

        let mut stream = res.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    *status.lock().unwrap() = StreamUiStatus::Error;
                    *last_err.lock().unwrap() = Some(e.to_string());
                    ctx.request_repaint();
                    break;
                }
            };
            let piece = String::from_utf8_lossy(&chunk);
            let (new_buf, messages) = feed_json_chunks(&buf, &piece);
            buf = new_buf;

            let mut changed = false;
            {
                let mut map = prices.lock().unwrap();
                for msg in messages {
                    let Some(obj) = msg.as_object() else {
                        continue;
                    };
                    if obj.contains_key("heartbeat") {
                        continue;
                    }
                    if obj.get("f").and_then(|v| v.as_str()) != Some("t") {
                        continue;
                    }
                    let Some(sym) = obj.get("i").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let Some(p) = obj.get("p").and_then(price_field_as_f64) else {
                        continue;
                    };
                    let Some(t) = obj.get("t").and_then(time_field_as_i64) else {
                        continue;
                    };
                    let t = event_time_to_unix_sec(t);
                    let price = decode_chainlink_price(p);
                    map.insert(sym.to_string(), LastPrice { price, t });
                    changed = true;
                }
            }
            if changed {
                ctx.request_repaint();
            }
        }

        *status.lock().unwrap() = StreamUiStatus::Reconnecting;
        ctx.request_repaint();
        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}
