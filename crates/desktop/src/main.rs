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

/// Maps to Candlestick `login` / `password` form fields (see Chainlink Candlestick API docs).
fn read_credentials() -> Option<(String, String)> {
    let user_id = strip_env_value(std::env::var("CHAINLINK_USER_ID").unwrap_or_default());
    let api_key = strip_env_value(std::env::var("CHAINLINK_API_KEY").unwrap_or_default());
    if user_id.is_empty() {
        eprintln!(
            "Missing CHAINLINK_USER_ID: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    if api_key.is_empty() {
        eprintln!(
            "Missing CHAINLINK_API_KEY: set it in the environment or in `.env` (see `.env.example`)."
        );
        return None;
    }
    Some((user_id, api_key))
}

fn main() -> eframe::Result<()> {
    load_dotenv();
    let Some((user_id, api_key)) = read_credentials() else {
        std::process::exit(1);
    };
    let price_api_base = read_price_api_base();

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
                user_id.clone(),
                api_key.clone(),
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
