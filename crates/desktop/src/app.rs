use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use egui::epaint::CornerRadiusF32;
use egui::{
    emath::Rangef, pos2, vec2, Color32, CursorIcon, FontId, LayerId, Order, PointerButton, Rect,
    Shape, Stroke, Vec2b,
};
use egui_plot::{
    uniform_grid_spacer, CoordinatesFormatter, Corner, GridInput, GridMark, HLine, LineStyle, Plot,
    PlotBounds, PlotPoint, PlotResponse, PlotUi,
};
use reqwest::Client;

use crate::assets::ASSET_LIST;
use crate::bff::{self, HistoryRowsResponse};
use crate::chart::{self, FormingBarState, SealedCandleRow};
use crate::stream::{self, LastPrice, StreamUiStatus};
use crate::unix_time;
use tokio::runtime::Handle;

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Подписи сетки по X: плотность под масштаб (plot x = Unix-секунды UTC).
fn format_plot_time_axis(mark: GridMark, range: &RangeInclusive<f64>) -> String {
    let s = (mark.value.round() as i64).clamp(i64::MIN, i64::MAX);
    let w = (range.end() - range.start()).max(1.0);
    unix_time::format_axis_label_utc(s, w)
}

/// Наименьшее из ряда 1, 2, 5×10ᵏ, … ≥ `m` — для «красивого» числа баров между линиями сетки.
fn nice_ceiled_unit(m: f64) -> f64 {
    let m = m.max(1e-30);
    if m <= 1.0 {
        return 1.0;
    }
    let log10 = m.log10().floor();
    let base = 10f64.powf(log10);
    for mult in [1.0, 2.0, 5.0, 10.0] {
        let cand = base * mult;
        if cand + f64::EPSILON >= m {
            return cand;
        }
    }
    base * 10.0
}

/// Три шага по оси X (секунды), все кратны `bar_secs` — линии совпадают с открытием бакетов свечей.
fn bar_synced_x_grid_steps(min_step_secs: f64, bar_secs: f64) -> [f64; 3] {
    let bar = bar_secs.max(f64::EPSILON);
    let k = nice_ceiled_unit((min_step_secs / bar).ceil().max(1.0));
    let s0 = k * bar;
    [s0, s0 * 10.0, s0 * 100.0]
}

/// Must match `Plot::y_axis_min_width` (extra space for wide tick labels).
const CHART_Y_AXIS_MIN_WIDTH: f32 = 132.0;
const PRICE_SCALE_LABEL_PAD: f32 = 40.0;

/// Минимум px между линиями сетки (как у дефолта `egui_plot::Plot::grid_spacing`); задан явно вместе с X-spacer.
const CHART_GRID_MIN_SPACING_PX: f32 = 8.0;
/// Зазор между телами свечей по X в px — тот же порядок, что `gap_x` в `box_plot_from_history`.
const CHART_CANDLE_BODY_GAP_PX: f32 = 1.0;

/// TradingView-style: vertical drag on the **left price strip** (next to, but not in, the plot area)
/// zooms the Y scale; the main plot area pans horizontally and vertically.
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
    if !ctx.input(|i| i.pointer.button_down(PointerButton::Primary)) {
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

/// Map mouse / trackpad scroll to **horizontal (time) zoom** only. Default `egui_plot` scroll pans;
/// we turn that off with `allow_scroll(false)` and apply X zoom here (see `allow_zoom` for pinch/Ctrl+wheel).
fn apply_time_axis_scroll_zoom(plot_ui: &mut PlotUi) {
    let d = plot_ui.ctx().input(|i| i.smooth_scroll_delta);
    if d == egui::Vec2::ZERO {
        return;
    }
    let pr = plot_ui.response().rect;
    let mut hit = pr;
    // Include the y-axis label column so scroll works over prices too
    hit.min.x -= CHART_Y_AXIS_MIN_WIDTH + PRICE_SCALE_LABEL_PAD;
    let Some(pos) = plot_ui.ctx().input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if !hit.contains(pos) {
        return;
    }
    // wheel / trackpad: use vertical scroll mainly; add horizontal for trackpads
    let scroll = d.y + 0.3 * d.x;
    if scroll == 0.0 {
        return;
    }
    // zoom_factor > 1 zooms in on the axis in plot space = narrower time range
    // Down-scroll (typ. +y) → zoom out (see more time) — sign inverted vs default feel
    let s = (1.0 + 0.0008 * scroll).clamp(0.92, 1.08);
    if (s - 1.0).abs() < f32::EPSILON {
        return;
    }
    plot_ui.zoom_bounds_around_hovered(egui::vec2(s, 1.0));
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
    let box_size = vec2(ts.x + 2.0 * PRICE_TAG_PAD, ts.y + 2.0 * PRICE_TAG_PAD);
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
    p.add(Shape::line_segment([p1, p2], Stroke::new(1.0, line_color)));
    p.rect_filled(tag, CornerRadiusF32::same(3.0), line_color);
    p.galley_with_override_text_color(
        pos2(tag.min.x + PRICE_TAG_PAD, tag.min.y + PRICE_TAG_PAD),
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
        /// Аналог `previousFormingRef` в веб-клиенте: бакет, которого ещё нет в API.
        forming_bar: Option<FormingBarState>,
        /// Последний merged-ряд при тике в последнем бакете API — подставляется при открытии нового бара.
        sealed_last_row: Option<SealedCandleRow>,
        /// Завершённые live-свечи (перешли из forming при смене бакета), ещё не подтверждённые REST.
        live_bars: Vec<FormingBarState>,
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
            let body =
                bff::fetch_history(&client, &base, &api_symbol, resolution.as_str(), from, now)
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
            forming_bar: None,
            sealed_last_row: None,
            live_bars: Vec::new(),
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
            forming_bar,
            sealed_last_row,
            live_bars,
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
                        *forming_bar = None;
                        *sealed_last_row = None;
                        live_bars.clear();
                        history_refresh = Some((api_symbol.clone(), r, history.clone()));
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
                    "Last price (stream): {:.6}   @ {}",
                    p.price,
                    unix_time::format_compact_utc(p.t)
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
                        let bar_secs = resolution.bar_seconds() as i64;
                        let bar_secs_f = resolution.bar_seconds();
                        let candles: Vec<Vec<f64>> = if let Some(p) = &last {
                            chart::merge_history_with_live(
                                &h.candles,
                                p.price,
                                p.t,
                                bar_secs,
                                forming_bar,
                                sealed_last_row,
                                live_bars,
                            )
                        } else {
                            chart::display_candles_without_live_tick(
                                &h.candles,
                                sealed_last_row,
                                forming_bar,
                                bar_secs,
                                live_bars,
                            )
                        };
                        let (x_lo, x_hi) = chart::candle_row_time_range(&candles);
                        // egui_plot: HLine не задаёт X в bounds(); явно растягиваем min/max по времени свечей.
                        let x_pad = bar_secs_f * 0.6;
                        let hline_stroke = last
                            .as_ref()
                            .and_then(|_| chart::last_candle_stroke_color(&candles, bar_secs));
                        if chart::ohlc_chart_has_data(&candles) {
                            let mut plot = Plot::new("ohlc_candles")
                                // With no fixed height, Plot fills remaining CentralPanel space; price
                                // axis and candle area stretch with the window. Y axis default
                                // min_thickness ~14px can clip price labels. Labels align to the axis
                                // strip; trailing spaces nudge the number left; Unicode em-spaces in
                                // the formatter add gap before candles.
                                .y_axis_min_width(CHART_Y_AXIS_MIN_WIDTH)
                                .x_axis_formatter(format_plot_time_axis)
                                .y_axis_formatter(|mark: GridMark, _range: &RangeInclusive<f64>| {
                                    format!("{:.2}   ", mark.value)
                                })
                                // Подсказка при наведении: время UTC, не сырой timestamp по X
                                .coordinates_formatter(
                                    Corner::LeftBottom,
                                    CoordinatesFormatter::new(
                                        |value: &PlotPoint, _bounds: &PlotBounds| {
                                            let t = value.x.round() as i64;
                                            format!(
                                                "{}  |  {:.2}",
                                                unix_time::format_compact_utc(t),
                                                value.y
                                            )
                                        },
                                    ),
                                )
                                // Pan X+Y; left price strip still uses vertical drag for Y zoom (see `apply_price_scale_y_zoom`).
                                .allow_drag(Vec2b { x: true, y: true })
                                // Pinch / Ctrl+wheel: time axis only; Y scale uses the price column drag.
                                .allow_zoom(Vec2b { x: true, y: false })
                                // Replaced with `apply_time_axis_scroll_zoom` (X zoom) instead of pan.
                                .allow_scroll(Vec2b { x: false, y: false })
                                .grid_spacing(Rangef::new(CHART_GRID_MIN_SPACING_PX, 300.0))
                                // X: шаги кратны `bar_secs` (как у `bar_time_for_timestamp`), плюс учёт 1px зазора тел.
                                .x_grid_spacer({
                                    let inner = uniform_grid_spacer({
                                        let bar = bar_secs_f;
                                        move |input: GridInput| {
                                            let scale = f64::from(
                                                (CHART_GRID_MIN_SPACING_PX
                                                    + CHART_CANDLE_BODY_GAP_PX)
                                                    / CHART_GRID_MIN_SPACING_PX,
                                            );
                                            bar_synced_x_grid_steps(
                                                input.base_step_size * scale,
                                                bar,
                                            )
                                        }
                                    });
                                    move |input: GridInput| inner(input)
                                });
                            if let (Some(lo), Some(hi)) = (x_lo, x_hi) {
                                plot = plot
                                    .include_x((lo as f64) - x_pad)
                                    .include_x((hi as f64) + x_pad);
                            }
                            let plot_response = plot.show(ui, |plot_ui| {
                                // 1 screen px → plot X (seconds); `dvalue_dpos` = plot units per ui point
                                let gap_x = plot_ui.transform().dvalue_dpos()[0].abs();
                                if let Some(box_plot) =
                                    chart::box_plot_from_history(&candles, bar_secs_f, gap_x)
                                {
                                    plot_ui.add(box_plot);
                                }
                                if let Some(p) = last.as_ref() {
                                    let col = hline_stroke
                                        .unwrap_or_else(|| Color32::from_rgb(100, 180, 255));
                                    plot_ui.hline(
                                        HLine::new("Current price", p.price)
                                            .color(col)
                                            .width(1.0)
                                            .style(LineStyle::dashed_loose()),
                                    );
                                }
                                apply_time_axis_scroll_zoom(plot_ui);
                                apply_price_scale_y_zoom(plot_ui);
                            });
                            if let Some(p) = last.as_ref() {
                                let tag_color = hline_stroke
                                    .unwrap_or_else(|| Color32::from_rgb(100, 180, 255));
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
            if let Screen::Detail {
                forming_bar,
                sealed_last_row,
                live_bars,
                ..
            } = &mut self.screen
            {
                *forming_bar = None;
                *sealed_last_row = None;
                live_bars.clear();
            }
            self.schedule_history_fetch(sym, r, slot);
        }
    }
}

impl eframe::App for ChainlinkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| match &self.screen {
            Screen::List => self.ui_list(ui),
            Screen::Detail { .. } => self.ui_detail(ui),
        });
    }
}
