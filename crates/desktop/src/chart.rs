//! Свечной график (OHLC) через `egui_plot::BoxPlot`, как в [issue egui #967](https://github.com/emilk/egui/issues/967).

use std::collections::HashMap;

use egui::Color32;
use egui::Stroke;
use egui_plot::{BoxElem, BoxPlot, BoxSpread};

use crate::price::{decode_chainlink_price, encode_chainlink_price};

/// Копия `candles` с обновлённой последней свечой (макс. `t`): close = `live`, high/low — расширяются.
pub fn candles_with_live_last(candles: &[Vec<f64>], live: f64) -> Vec<Vec<f64>> {
    if candles.is_empty() {
        return vec![];
    }
    let Some(idx) = last_candle_row_index(candles) else {
        return candles.to_vec();
    };

    let mut out: Vec<Vec<f64>> = candles.to_vec();
    {
        let row = &mut out[idx];
        let h = decode_chainlink_price(row[2]);
        let l = decode_chainlink_price(row[3]);
        let c = live;
        let h2 = h.max(c);
        let l2 = l.min(c);
        row[2] = encode_chainlink_price(h2);
        row[3] = encode_chainlink_price(l2);
        row[4] = encode_chainlink_price(c);
    }
    out
}

/// `stroke` той же свечи, что рисуется справа (макс. `t`), с тем же `normalize_ohlc_display` что и `box_plot_from_history`.
pub fn last_candle_stroke_color(candles: &[Vec<f64>]) -> Option<Color32> {
    let idx = last_candle_row_index(candles)?;
    let row = &candles[idx];
    if row.len() < 5 {
        return None;
    }
    let o = decode_chainlink_price(row[1]);
    let h = decode_chainlink_price(row[2]);
    let l = decode_chainlink_price(row[3]);
    let c = decode_chainlink_price(row[4]);
    let (o, _h, _l, c) = normalize_ohlc_display(o, h, l, c);
    let bullish = c >= o;
    Some(if bullish {
        Color32::from_rgb(46, 160, 67)
    } else {
        Color32::from_rgb(200, 64, 64)
    })
}

fn last_candle_row_index(candles: &[Vec<f64>]) -> Option<usize> {
    let (idx, _) = candles
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            if row.len() >= 5 {
                Some((i, row[0] as i64))
            } else {
                None
            }
        })
        .max_by_key(|(_, t)| *t)?;
    Some(idx)
}

#[inline]
fn min_price_spread(anchor: f64) -> f64 {
    (anchor.abs() * 1e-5).max(0.01)
}

/// Упрощённый аналог `normalizeCandleForDisplay` из `src/lib/chainlink/candles.ts`.
fn normalize_ohlc_display(open: f64, high: f64, low: f64, close: f64) -> (f64, f64, f64, f64) {
    let ref_ = (high + low) / 2.0;
    let anchor = if ref_.abs() > f64::EPSILON { ref_ } else { close };
    let spread = min_price_spread(anchor);
    let mut high = high;
    let mut low = low;
    if (high - low).abs() < spread {
        let m = (high + low) / 2.0;
        let half = spread / 2.0;
        high = m + half;
        low = m - half;
    }
    let range = high - low;
    let mut open = open;
    let mut close = close;
    if range > 0.0 {
        let mid_oc = (open + close) / 2.0;
        let body_spread = min_price_spread(mid_oc);
        if (open - close).abs() < body_spread {
            let m = mid_oc;
            let body_half = (min_price_spread(m) * 0.2)
                .max(range * 0.02)
                .min(range * 0.45);
            if 2.0 * body_half < range {
                open = m - body_half;
                close = m + body_half;
            }
        }
    }
    (open, high, low, close)
}

/// `bar_secs` — длина бара в секундах (300 для 5m, 900 для 15m) для ширины тела на оси времени.
pub fn box_plot_from_history(candles: &[Vec<f64>], bar_secs: f64) -> Option<BoxPlot> {
    let mut by_t: HashMap<i64, (f64, f64, f64, f64)> = HashMap::new();

    for row in candles {
        if row.len() < 5 {
            continue;
        }
        let t = row[0] as i64;
        let o = decode_chainlink_price(row[1]);
        let h = decode_chainlink_price(row[2]);
        let l = decode_chainlink_price(row[3]);
        let c = decode_chainlink_price(row[4]);
        let (o, h, l, c) = normalize_ohlc_display(o, h, l, c);
        by_t.insert(t, (o, h, l, c));
    }

    if by_t.is_empty() {
        return None;
    }

    let mut times: Vec<i64> = by_t.keys().copied().collect();
    times.sort_unstable();

    let box_width = (bar_secs * 0.55).clamp(30.0, bar_secs * 0.95);

    let mut boxes = Vec::with_capacity(times.len());
    for t in times {
        let (o, h, l, c) = by_t[&t];
        let lo = o.min(c);
        let hi = o.max(c);
        let mid = (o + c) / 2.0;
        let bullish = c >= o;
        let fill = if bullish {
            Color32::from_rgb(46, 160, 67)
        } else {
            Color32::from_rgb(200, 64, 64)
        };
        let stroke = Stroke::new(1.0, fill);

        let spread = BoxSpread::new(l, lo, mid, hi, h);
        let elem = BoxElem::new(t as f64, spread)
            .whisker_width(0.0)
            .box_width(box_width)
            .fill(fill.linear_multiply(0.35))
            .stroke(stroke)
            .vertical();
        boxes.push(elem);
    }

    Some(BoxPlot::new(boxes).vertical().name("OHLC"))
}
