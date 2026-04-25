use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use egui::{
    pos2, vec2, Color32, CursorIcon, FontId, LayerId, Order, PointerButton, Rect, Rounding, Shape,
    Stroke, Vec2b,
};
use egui_plot::{GridMark, HLine, LineStyle, Plot, PlotPoint, PlotResponse, PlotUi};
use reqwest::Client;

use crate::assets::ASSET_LIST;
use crate::chart;
use crate::bff::{self, HistoryRowsResponse};
use crate::stream::{self, LastPrice, StreamUiStatus};
use tokio::runtime::Handle;

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Must match `Plot::y_axis_min_width` (extra space for wide tick labels).
const CHART_Y_AXIS_MIN_WIDTH: f32 = 132.0;
const PRICE_SCALE_LABEL_PAD: f32 = 40.0;

/// TradingView-style: vertical drag on the **left price strip** (next to, but not in, the plot area)
/// zooms the Y scale; the main area pans time only (no vertical pan from drag).
fn apply_price_scale_y_zoom(plot_ui: &mut PlotUi) {
    let pr = plot_ui.response().rect;
    let left = pr.left() - (CHART_Y_AXIS_MIN_WIDTH + PRICE_SCALE_LABEL_PAD);
    let in_strip = |pos: egui::Pos2| {
        (left..=pr.left()).contains(&pos.x) && (pr.min.y..=pr.max.y).contains(&pos.y)
    };
    let ctx = plot_ui.ctx().clone();
    let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    if !in_strip(pos) {
        return;
    }
    ctx.set_cursor_icon(CursorIcon::ResizeVertical);
    if !ctx
        .input(|i| i.pointer.button_down(PointerButton::Primary))
    {
        return;
    }
    let dy: f32 = ctx.input(|i| i.pointer.delta().y);
    if dy == 0.0 {
        return;
    }
    // Drag up (smaller screen y) → larger zoom factor = narrower y-range (see `PlotBounds::zoom`)
    // Drag down → smaller factor = wider y-range (like TradingView price scale)
    let s = (1.0 - 0.0018 * dy).clamp(0.92, 1.08);
    let center = plot_ui.plot_bounds().center();
    plot_ui.zoom_bounds(egui::vec2(1.0, s), center);
}

const PRICE_TAG_PAD: f32 = 5.0;
const PRICE_TAG_GAP: f32 = 4.0;

/// Rounded price tag in the y-axis area (left of the plot), on top of tick text; short horizontal
/// segment links the tag to the left edge of the inner plot, aligned with the HLine.
fn paint_current_price_y_axis_tag(
    ui: &egui::Ui,
    plot: &PlotResponse<()>,
    price: f64,
    line_color: Color32,
) {
    let t = &plot.transform;
    let b = t.bounds();
    if !b.is_valid_y() {
        return;
    }
    let x_mid = (b.min()[0] + b.max()[0]) * 0.5;
    let p_screen = t.position_from_point(&PlotPoint::new(x_mid, price));
    let pr = plot.response.rect;
    let y_clamped = p_screen.y.clamp(
        pr.min.y + 8.0f32,
        (pr.max.y - 8.0f32).max(pr.min.y + 8.0f32),
    );
    let text = format!("{:.2}", price);
    let font = FontId::proportional(13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text, font, Color32::PLACEHOLDER);
    let ts = galley.size();
    let box_size = vec2(
        ts.x + 2.0 * PRICE_TAG_PAD,
        ts.y + 2.0 * PRICE_TAG_PAD,
    );
    let right = pr.min.x - PRICE_TAG_GAP;
    let tag = Rect::from_min_size(
        pos2(right - box_size.x, y_clamped - 0.5 * box_size.y),
        box_size,
    );
    let line_y = tag.center().y;
    let p1 = pos2(tag.max.x, line_y);
    let p2 = pos2(pr.min.x, line_y);
    let layer = LayerId::new(Order::Tooltip, ui.id().with("cur_price_badge"));
    let mut p = ui.ctx().layer_painter(layer);
    p.set_clip_rect(ui.clip_rect());
    p.add(Shape::line_segment(
        [p1, p2],
        Stroke::new(1.5, line_color),
    ));
    p.rect_filled(tag, Rounding::same(3.0), line_color);
    p.galley_with_override_text_color(
        pos2(
            tag.min.x + PRICE_TAG_PAD,
            tag.min.y + PRICE_TAG_PAD,
        ),
        galley,
        Color32::WHITE,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    M5,
    M15,
}

impl Resolution {
    fn as_str(self) -> &'static str {
        match self {
            Resolution::M5 => "5m",
            Resolution::M15 => "15m",
        }
    }

    /// Bar length in seconds (candle width on the time axis).
    fn bar_seconds(self) -> f64 {
        match self {
            Resolution::M5 => 300.0,
            Resolution::M15 => 900.0,
        }
    }
}

type HistorySlot = Arc<Mutex<Option<Result<HistoryRowsResponse, String>>>>;

pub enum Screen {
    List,
    Detail {
        label: String,
        api_symbol: String,
        resolution: Resolution,
        history: HistorySlot,
    },
}

pub struct ChainlinkApp {
    base_url: String,
    runtime: Handle,
    client: Client,
    egui_ctx: egui::Context,
    stream_prices: Arc<Mutex<HashMap<String, LastPrice>>>,
    stream_status: Arc<Mutex<StreamUiStatus>>,
    stream_err: Arc<Mutex<Option<String>>>,
    screen: Screen,
}

impl ChainlinkApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Handle) -> Self {
        let base_url = std::env::var("CHAINLINK_CHARTS_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");

        // Long-lived stream: avoid per-request client default timeout.
        let stream_client = Client::builder()
            .timeout(Duration::MAX)
            .build()
            .expect("stream reqwest client");

        let egui_ctx = cc.egui_ctx.clone();
        let stream_prices = Arc::new(Mutex::new(HashMap::new()));
        let stream_status = Arc::new(Mutex::new(StreamUiStatus::Connecting));
        let stream_err = Arc::new(Mutex::new(None));

        runtime.spawn(stream::stream_loop(
            base_url.clone(),
            stream_client,
            egui_ctx.clone(),
            stream_prices.clone(),
            stream_status.clone(),
            stream_err.clone(),
        ));

        Self {
            base_url,
            runtime,
            client,
            egui_ctx,
            stream_prices,
            stream_status,
            stream_err,
            screen: Screen::List,
        }
    }

    fn schedule_history_fetch(
        &self,
        api_symbol: String,
        resolution: Resolution,
        slot: HistorySlot,
    ) {
        *slot.lock().expect("history lock") = None;
        let client = self.client.clone();
        let base = self.base_url.clone();
        let ctx = self.egui_ctx.clone();
        self.runtime.spawn(async move {
            let now = unix_now_secs();
            let from = now - 86400;
            let body = bff::fetch_history(
                &client,
                &base,
                &api_symbol,
                resolution.as_str(),
                from,
                now,
            )
            .await
            .map_err(|e| e.to_string());
            *slot.lock().expect("history slot") = Some(body);
            ctx.request_repaint();
        });
    }

    fn open_detail(&mut self, label: String, api_symbol: String) {
        let history = Arc::new(Mutex::new(None));
        let resolution = Resolution::M5;
        self.schedule_history_fetch(api_symbol.clone(), resolution, history.clone());
        self.screen = Screen::Detail {
            label,
            api_symbol,
            resolution,
            history,
        };
    }

    fn stream_status_label(status: StreamUiStatus) -> &'static str {
        match status {
            StreamUiStatus::Connecting => "Stream: connecting…",
            StreamUiStatus::Live => "Stream: live",
            StreamUiStatus::Reconnecting => "Stream: reconnecting…",
            StreamUiStatus::Error => "Stream: error",
            StreamUiStatus::Unconfigured => "Server not configured (503)",
        }
    }

    fn ui_list(&mut self, ui: &mut egui::Ui) {
        let st = *self.stream_status.lock().expect("status");
        ui.label(Self::stream_status_label(st));
        if let Some(e) = self.stream_err.lock().expect("err").as_deref() {
            ui.colored_label(egui::Color32::RED, e);
        }
        if st == StreamUiStatus::Unconfigured {
            ui.label("Set Chainlink environment variables in Next.js and restart the BFF.");
        }
        ui.separator();
        ui.heading("Assets");
        for row in ASSET_LIST {
            if ui
                .button(format!("{} — {}", row.label, row.api_symbol))
                .clicked()
            {
                self.open_detail(row.label.to_string(), row.api_symbol.to_string());
            }
        }
    }

    fn ui_detail(&mut self, ui: &mut egui::Ui) {
        let mut go_back = false;
        let mut history_refresh: Option<(String, Resolution, HistorySlot)> = None;

        if let Screen::Detail {
            label,
            api_symbol,
            resolution,
            history,
        } = &mut self.screen
        {
            ui.horizontal(|ui| {
                if ui.button("← Back to list").clicked() {
                    go_back = true;
                }
            });
            ui.separator();
            ui.heading(format!("{label} ({api_symbol})"));

            ui.horizontal(|ui| {
                ui.label("Interval:");
                for (r, lbl) in [(Resolution::M5, "5m"), (Resolution::M15, "15m")] {
                    if ui.selectable_label(*resolution == r, lbl).clicked() && *resolution != r {
                        *resolution = r;
                        history_refresh =
                            Some((api_symbol.clone(), r, history.clone()));
                    }
                }
            });

            let last = self
                .stream_prices
                .lock()
                .expect("prices")
                .get(api_symbol.as_str())
                .cloned();
            if let Some(ref p) = last {
                ui.label(format!(
                    "Last price (stream): {:.6}  (t={})",
                    p.price, p.t
                ));
            } else {
                ui.label("No tick for this symbol in the current stream (waiting…)");
            }

            ui.separator();
            match history.lock().expect("hist").as_ref() {
                None => {
                    ui.spinner();
                    ui.label("Loading history…");
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::RED, format!("History error: {e}"));
                }
                Some(Ok(h)) => {
                    if h.candles.is_empty() {
                        ui.label("No candles for the selected period.");
                    } else {
                        let (box_plot, hline_stroke) = match &last {
                            Some(p) => {
                                let candles = chart::candles_with_live_last(&h.candles, p.price);
                                (
                                    chart::box_plot_from_history(
                                        &candles,
                                        resolution.bar_seconds(),
                                    ),
                                    chart::last_candle_stroke_color(&candles),
                                )
                            }
                            None => (
                                chart::box_plot_from_history(
                                    &h.candles,
                                    resolution.bar_seconds(),
                                ),
                                None,
                            ),
                        };
                        if let Some(box_plot) = box_plot {
                            let plot_response = Plot::new("ohlc_candles")
                                // With no fixed height, Plot fills remaining CentralPanel space; price
                                // axis and candle area stretch with the window. Y axis default
                                // min_thickness ~14px can clip price labels. Labels align to the axis
                                // strip; trailing spaces nudge the number left; Unicode em-spaces in
                                // the formatter add gap before candles.
                                .y_axis_min_width(CHART_Y_AXIS_MIN_WIDTH)
                                .y_axis_formatter(
                                    |mark: GridMark, _range: &RangeInclusive<f64>| {
                                        format!("{:.2}   ", mark.value)
                                    },
                                )
                                // Pan in time only; vertical drag on the main area does not shift Y (see price strip below).
                                .allow_drag(Vec2b {
                                    x: true,
                                    y: false,
                                })
                                .allow_zoom(true)
                                .allow_scroll(true)
                                .show(ui, |plot_ui| {
                                    plot_ui.add(box_plot);
                                    if let Some(p) = last.as_ref() {
                                        let col = hline_stroke.unwrap_or_else(|| {
                                            Color32::from_rgb(100, 180, 255)
                                        });
                                        plot_ui.hline(
                                            HLine::new(p.price)
                                                .name("Current price")
                                                .color(col)
                                                .width(1.5)
                                                .style(LineStyle::dashed_loose()),
                                        );
                                    }
                                    apply_price_scale_y_zoom(plot_ui);
                                });
                            if let Some(p) = last.as_ref() {
                                let tag_color = hline_stroke.unwrap_or_else(|| {
                                    Color32::from_rgb(100, 180, 255)
                                });
                                paint_current_price_y_axis_tag(
                                    ui,
                                    &plot_response,
                                    p.price,
                                    tag_color,
                                );
                            }
                    } else {
                        ui.label("Could not build candles from the API response.");
                    }
                    }
                }
            }
        }

        if go_back {
            self.screen = Screen::List;
        }
        if let Some((sym, r, slot)) = history_refresh {
            self.schedule_history_fetch(sym, r, slot);
        }
    }
}

impl eframe::App for ChainlinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match &self.screen {
                Screen::List => self.ui_list(ui),
                Screen::Detail { .. } => self.ui_detail(ui),
            }
        });
    }
}
