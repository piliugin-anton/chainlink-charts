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

/// Maximum ticks held per symbol while the UI is not rendering (e.g. window minimised).
const MAX_PENDING_TICKS: usize = 1000;

fn feed_id_to_symbol(id: &ID) -> Option<String> {
    let hex = id.to_hex_string();
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
        user_id.clone(),
        api_key.clone(),
        SDK_REST_URL.to_string(),
        SDK_WS_URL.to_string(),
    )
    .build()
    {
        Ok(c) => c,
        Err(e) => {
            *status.lock().unwrap() = StreamUiStatus::Unconfigured;
            *last_err.lock().unwrap() = Some(e.to_string());
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
            Err(StreamError::AuthError(_)) => {
                *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                *last_err.lock().unwrap() = Some("credentials missing or invalid".into());
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
            sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
            continue;
        }

        *status.lock().unwrap() = StreamUiStatus::Live;
        backoff = Duration::from_secs(1);
        ctx.request_repaint();

        loop {
            match stream.read().await {
                Err(StreamError::AuthError(_)) => {
                    *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                    *last_err.lock().unwrap() = Some("credentials missing or invalid".into());
                    ctx.request_repaint();
                    let _ = stream.close().await;
                    return;
                }
                Err(e) => {
                    *status.lock().unwrap() = StreamUiStatus::Error;
                    *last_err.lock().unwrap() = Some(e.to_string());
                    ctx.request_repaint();
                    let _ = stream.close().await;
                    break;
                }
                Ok(ws_report) => {
                    let report = &ws_report.report;
                    let Some(sym) = feed_id_to_symbol(&report.feed_id) else {
                        continue;
                    };

                    let full_report_hex = report.full_report.trim_start_matches("0x");
                    let bytes = match hex::decode(full_report_hex) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };

                    let (_, blob) = match decode_full_report(&bytes) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let v3 = match ReportDataV3::decode(&blob) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    let raw_f64: f64 = v3.benchmark_price.to_string().parse().unwrap_or(0.0);
                    let price = decode_chainlink_price(raw_f64);
                    let t = event_time_to_unix_sec(v3.valid_from_timestamp as i64);

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
            }
        }

        *status.lock().unwrap() = StreamUiStatus::Reconnecting;
        ctx.request_repaint();
        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}
