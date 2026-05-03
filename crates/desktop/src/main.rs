mod app;
mod assets;
mod auth;
mod candlestick;
mod chart;
mod json_chunks;
mod price;
mod stream;
mod unix_time;

fn main() -> eframe::Result<()> {
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
        Box::new(move |cc| Ok(Box::new(app::ChainlinkApp::new(cc, handle)))),
    )
}
