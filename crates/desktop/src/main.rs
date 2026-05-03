mod app;
mod assets;
mod auth;
mod candlestick;
mod chart;
#[cfg(test)]
mod json_chunks;
mod price;
mod stream;
mod unix_time;

fn load_dotenv() {
    if let Err(e) = dotenvy::dotenv() {
        if e.not_found() {
            // `.env` is optional; variables may come only from the process environment.
            return;
        }
        eprintln!("Warning: could not load `.env`: {e}");
    }
}

fn strip_env_value(s: String) -> String {
    // `.trim()` does not remove UTF-8 BOM; dotenv / editors may leave U+FEFF at the start of a line.
    s.trim().trim_start_matches('\u{feff}').to_string()
}

/// Candlestick / authorize base URL (testnet or mainnet), without trailing slash.
fn read_price_api_base() -> String {
    let raw = std::env::var("CHAINLINK_BASE_URL").unwrap_or_default();
    let trimmed = strip_env_value(raw).trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return "https://priceapi.dataengine.chain.link".to_string();
    }
    if !trimmed.to_ascii_lowercase().contains("priceapi") {
        eprintln!(
            "Warning: CHAINLINK_BASE_URL should be the Candlestick price API host (URL containing \"priceapi\", e.g. …testnet-dataengine… or …dataengine…). \
A JWT from `curl …/authorize` is only valid for `…/history` on that same host."
        );
    }
    trimmed
}

/// Data Streams SDK REST/WS hosts must match the Candlestick environment (testnet vs mainnet).
/// WebSocket base URL per Chainlink docs: [Data Streams WebSocket](https://docs.chain.link/data-streams/reference/data-streams-api/interface-ws)
/// (`wss://ws…dataengine…`, not `priceapi…`).
fn data_streams_engine_urls(price_api_base: &str) -> (String, String) {
    let lower = price_api_base.to_ascii_lowercase();
    if lower.contains("testnet") {
        (
            "https://api.testnet-dataengine.chain.link".to_string(),
            "wss://ws.testnet-dataengine.chain.link".to_string(),
        )
    } else {
        (
            "https://api.dataengine.chain.link".to_string(),
            "wss://ws.dataengine.chain.link".to_string(),
        )
    }
}

/// Maps to Candlestick `login` / `password` form fields (see Chainlink Candlestick API docs).
fn read_candlestick_credentials() -> Option<(String, String)> {
    let candlestick_user_id =
        strip_env_value(std::env::var("CHAINLINK_USER_ID").unwrap_or_default());
    let candlestick_api_key =
        strip_env_value(std::env::var("CHAINLINK_API_KEY").unwrap_or_default());
    if candlestick_user_id.is_empty() {
        eprintln!(
            "Missing CHAINLINK_USER_ID: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    if candlestick_api_key.is_empty() {
        eprintln!(
            "Missing CHAINLINK_API_KEY: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    Some((candlestick_user_id, candlestick_api_key))
}

/// Data Engine `Authorization` must be the **client UUID**; the other value is the HMAC secret.
/// If `.env` swaps them, the WS handshake often returns HTTP 400 (see working example:
/// <https://github.com/piliugin-anton/polymarket-crypto/blob/master/src/feeds/chainlink_data_streams.rs>).
fn normalize_stream_credentials(raw_key: &str, raw_secret: &str) -> (String, String) {
    let k = raw_key.trim();
    let s = raw_secret.trim();
    let key_is_uuid = uuid::Uuid::parse_str(k).is_ok();
    let secret_is_uuid = uuid::Uuid::parse_str(s).is_ok();
    if !key_is_uuid && secret_is_uuid {
        eprintln!(
            "Warning: CHAINLINK_STREAM_API_KEY is not a UUID but CHAINLINK_STREAM_API_SECRET parses as one. \
Using swapped order: Data Streams expects the UUID in `api_key` (Authorization header) and the longer secret for HMAC."
        );
        (s.to_string(), k.to_string())
    } else {
        (k.to_string(), s.to_string())
    }
}

/// Data Streams SDK `Config::new(api_key, api_secret, …)` — separate from Candlestick JWT credentials.
fn read_stream_credentials() -> Option<(String, String)> {
    let stream_api_key =
        strip_env_value(std::env::var("CHAINLINK_STREAM_API_KEY").unwrap_or_default());
    let stream_api_secret =
        strip_env_value(std::env::var("CHAINLINK_STREAM_API_SECRET").unwrap_or_default());
    if stream_api_key.is_empty() {
        eprintln!(
            "Missing CHAINLINK_STREAM_API_KEY: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    if stream_api_secret.is_empty() {
        eprintln!(
            "Missing CHAINLINK_STREAM_API_SECRET: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    let (k, s) = normalize_stream_credentials(&stream_api_key, &stream_api_secret);
    Some((k, s))
}

fn main() -> eframe::Result<()> {
    load_dotenv();
    let Some((candlestick_user_id, candlestick_api_key)) = read_candlestick_credentials() else {
        std::process::exit(1);
    };
    let Some((stream_api_key, stream_api_secret)) = read_stream_credentials() else {
        std::process::exit(1);
    };
    let price_api_base = read_price_api_base();
    let (sdk_rest_url, sdk_ws_url) = data_streams_engine_urls(&price_api_base);

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
        Box::new(move |cc| {
            Ok(Box::new(app::ChainlinkApp::new(
                cc,
                handle.clone(),
                price_api_base.clone(),
                sdk_rest_url.clone(),
                sdk_ws_url.clone(),
                candlestick_user_id.clone(),
                candlestick_api_key.clone(),
                stream_api_key.clone(),
                stream_api_secret.clone(),
            )))
        }),
    )
    .inspect_err(|e| {
        eprintln!("Failed to open a window: {e}");
        eprintln!(
            "A graphical session (Wayland or X11) is required. \
If you see a Wayland compositor error, try an X11 session or `unset WAYLAND_DISPLAY` when an X server is available."
        );
    })
}
