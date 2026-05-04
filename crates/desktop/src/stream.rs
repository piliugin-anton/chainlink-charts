use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chainlink_data_streams_report::feed_id::ID;
use chainlink_data_streams_report::report::decode_full_report;
use chainlink_data_streams_report::report::v3::ReportDataV3;
use chainlink_data_streams_sdk::client::Client as StreamsRestClient;
use chainlink_data_streams_sdk::config::Config;
use chainlink_data_streams_sdk::stream::{Stream, StreamError};
use tokio::time::sleep;

use crate::assets::ASSET_LIST;
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

/// Maximum ticks held per symbol while the UI is not rendering (e.g. window minimised).
const MAX_PENDING_TICKS: usize = 1000;

fn feed_id_to_symbol(id: &ID) -> Option<String> {
    let hex = id.to_hex_string();
    ASSET_LIST
        .iter()
        .find(|a| {
            hex.eq_ignore_ascii_case(a.feed_id_mainnet)
                || hex.eq_ignore_ascii_case(a.feed_id_testnet)
        })
        .map(|a| a.api_symbol.to_string())
}

/// The SDK may surface HTTP 401 as a generic `StreamError` (not `StreamError::AuthError`).
fn stream_err_is_http_401_unauthorized(e: &StreamError) -> bool {
    let s = e.to_string();
    s.contains("401") && s.to_ascii_lowercase().contains("unauthorized")
}

pub async fn stream_loop(
    stream_api_key: String,
    stream_api_secret: String,
    sdk_rest_url: String,
    sdk_ws_url: String,
    streams_testnet: bool,
    ctx: egui::Context,
    prices: Arc<Mutex<HashMap<String, LastPrice>>>,
    tick_queue: Arc<Mutex<HashMap<String, Vec<LastPrice>>>>,
    status: Arc<Mutex<StreamUiStatus>>,
    last_err: Arc<Mutex<Option<String>>>,
) {
    // SDK default `ws_max_reconnect` is 5; after that the background reader task exits.
    // `Stream` still holds an `mpsc::Sender`, so `read()` never returns `StreamClosed` and
    // our outer reconnect loop never runs — keep retrying inside the SDK instead.
    let config = match Config::new(
        stream_api_key.clone(),
        stream_api_secret.clone(),
        sdk_rest_url.clone(),
        sdk_ws_url.clone(),
    )
    .with_ws_max_reconnect(usize::MAX)
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

    let allowed_hex: Option<HashSet<String>> = match StreamsRestClient::new(config.clone()) {
        Ok(dc) => match dc.get_feeds().await {
            Ok(feeds) => {
                let set: HashSet<String> = feeds
                    .into_iter()
                    .map(|f| f.feed_id.to_hex_string().to_ascii_lowercase())
                    .collect();
                Some(set)
            }
            Err(_) => None,
        },
        Err(_) => None,
    };

    let candidates: Vec<ID> = ASSET_LIST
        .iter()
        .filter_map(|a| {
            let s = if streams_testnet {
                a.feed_id_testnet
            } else {
                a.feed_id_mainnet
            };
            ID::from_hex_str(s).ok()
        })
        .collect();

    let feed_ids: Vec<ID> = match &allowed_hex {
        Some(allowed) if !allowed.is_empty() => {
            let filtered: Vec<ID> = candidates
                .iter()
                .copied()
                .filter(|id| allowed.contains(&id.to_hex_string().to_ascii_lowercase()))
                .collect();
            if filtered.is_empty() {
                candidates.clone()
            } else {
                filtered
            }
        }
        _ => candidates.clone(),
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
                if stream_err_is_http_401_unauthorized(&e) {
                    *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                    *last_err.lock().unwrap() = Some(
                        "Data Streams returned HTTP 401 on connect. If your stream credentials are for testnet, set CHAINLINK_BASE_URL=https://priceapi.testnet-dataengine.chain.link (SDK follows that host).".into(),
                    );
                    ctx.request_repaint();
                    return;
                }
                *status.lock().unwrap() = StreamUiStatus::Error;
                *last_err.lock().unwrap() = Some(e.to_string());
                ctx.request_repaint();
                sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
                continue;
            }
        };

        if let Err(e) = stream.listen().await {
            if stream_err_is_http_401_unauthorized(&e) {
                *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                *last_err.lock().unwrap() = Some(
                    "Data Streams returned HTTP 401. Check CHAINLINK_STREAM_API_KEY / CHAINLINK_STREAM_API_SECRET and CHAINLINK_BASE_URL (testnet vs mainnet).".into(),
                );
                ctx.request_repaint();
                let _ = stream.close().await;
                return;
            }
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
                    if stream_err_is_http_401_unauthorized(&e) {
                        *status.lock().unwrap() = StreamUiStatus::Unconfigured;
                        *last_err.lock().unwrap() = Some(
                            "Data Streams returned HTTP 401. Check CHAINLINK_STREAM_API_KEY / CHAINLINK_STREAM_API_SECRET and CHAINLINK_BASE_URL (testnet vs mainnet).".into(),
                        );
                        ctx.request_repaint();
                        let _ = stream.close().await;
                        return;
                    }
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
