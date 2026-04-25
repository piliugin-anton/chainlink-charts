//! Свечной график (OHLC) через `egui_plot::BoxPlot`, как в [issue egui #967](https://github.com/emilk/egui/issues/967).

use std::collections::HashMap;

use egui::Color32;
use egui::Stroke;
use egui_plot::{BoxElem, BoxPlot, BoxSpread};

use crate::price::{decode_chainlink_price, encode_chainlink_price};
use crate::unix_time::event_time_to_unix_sec;

/// Начало интервала бара (сек), как `barTimeForTimestamp` в `candles.ts`.
#[inline]
pub fn bar_time_for_timestamp(ts_sec: i64, bar_secs: i64) -> i64 {
    (ts_sec / bar_secs) * bar_secs
}

/// Макс. `bar_time_for_timestamp` по рядам — согласовано с `box_plot_from_history` и с логикой merge.
fn max_aligned_bar_time(candles: &[Vec<f64>], bar_s: i64) -> Option<i64> {
    candles
        .iter()
        .filter_map(|row| (row.len() >= 5).then_some(bar_time_for_timestamp(row[0] as i64, bar_s)))
        .max()
}

/// Правая свеча в **самом правом** тайм-бакете (как на графике после `box_plot` с ключом по `bar_t`).
fn rightmost_in_highest_bar_bucket(candles: &[Vec<f64>], bar_s: i64) -> Option<usize> {
    let key_max = max_aligned_bar_time(candles, bar_s)?;
    (0..candles.len()).rev().find(|&i| {
        let r = &candles[i];
        r.len() >= 5 && bar_time_for_timestamp(r[0] as i64, bar_s) == key_max
    })
}

/// Диапазон сырых `t` по рядам (для `Plot::include_x` — у `HLine` в egui_plot нет вклада в X bounds).
pub fn candle_row_time_range(rows: &[Vec<f64>]) -> (Option<i64>, Option<i64>) {
    candle_time_extents(rows)
}

fn candle_time_extents(rows: &[Vec<f64>]) -> (Option<i64>, Option<i64>) {
    let mut min: Option<i64> = None;
    let mut max: Option<i64> = None;
    for r in rows {
        if r.len() < 5 {
            continue;
        }
        let x = r[0] as i64;
        min = Some(min.map_or(x, |m: i64| m.min(x)));
        max = Some(max.map_or(x, |m: i64| m.max(x)));
    }
    (min, max)
}

/// Состояние «формирующейся» свечи, которой ещё нет в ответе API (новый тайм-слот).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormingBarState {
    pub bar_t: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// Последняя нарисованная строка OHLC для `bar_t` (сырой вектор как в `history`, для `sealed` в
/// [`merge_history_with_live`]).
pub type SealedCandleRow = (i64, Vec<f64>);

/// История + live-тик: обновление последней свечи API или добавление новой, если тик в следующем `bar`.
///
/// Пока бакет в ответе есть (`bar == last_t` в API), сбрасывает [`FormingBarState`]. Пока бакета нет
/// (`bar > last_t`), копит OHLC в `forming` между кадрами, как `computeFormingBar` + ref в веб-клиенте.
///
/// `sealed_last` — копия **последнего** merged-ряда, пока тик в текущем бакете (при `bar==last_t`).
/// При `bar>last_t` она **подставляется** вместо последней строки `history` (её time совпадает с
/// `max` по API), чтобы «закрывшаяся» свеча не откатывалась к устаревшему REST и не визуально
/// скачивалась.
pub fn merge_history_with_live(
    history: &[Vec<f64>],
    price: f64,
    tick_t: i64,
    bar_secs: i64,
    forming: &mut Option<FormingBarState>,
    sealed_last: &mut Option<SealedCandleRow>,
) -> Vec<Vec<f64>> {
    let tick_t = event_time_to_unix_sec(tick_t);
    let bar = bar_time_for_timestamp(tick_t, bar_secs);
    let last_bar_t = max_aligned_bar_time(history, bar_secs).unwrap_or(i64::MIN);

    if bar < last_bar_t {
        return history.to_vec();
    }

    if bar == last_bar_t {
        *forming = None;
        let out = candles_with_live_last(history, price, bar_secs);
        if let Some(idx) = rightmost_in_highest_bar_bucket(&out, bar_secs) {
            let t_key = bar_time_for_timestamp(out[idx][0] as i64, bar_secs);
            *sealed_last = Some((t_key, out[idx].clone()));
        } else {
            *sealed_last = None;
        }
        return out;
    }

    // `bar` > `last_bar_t` — в ответе API нет бакета под текущий тик: зафиксировать закрытие прошлого.
    let mut out = history.to_vec();
    if let Some((st, row)) = sealed_last.as_ref() {
        if *st == last_bar_t {
            if let Some(j) = out.iter().rposition(|r| {
                r.len() >= 5 && bar_time_for_timestamp(r[0] as i64, bar_secs) == *st
            }) {
                out[j] = row.clone();
            }
        }
    }

    let f = match *forming {
        Some(prev) if prev.bar_t == bar => FormingBarState {
            bar_t: bar,
            open: prev.open,
            high: prev.high.max(price),
            low: prev.low.min(price),
            close: price,
        },
        _ => FormingBarState {
            bar_t: bar,
            open: price,
            high: price,
            low: price,
            close: price,
        },
    };
    *forming = Some(f);
    out.push(vec![
        f.bar_t as f64,
        encode_chainlink_price(f.open),
        encode_chainlink_price(f.high),
        encode_chainlink_price(f.low),
        encode_chainlink_price(f.close),
    ]);
    out
}

/// Пока нет live-тика: подставить `sealed` (последняя нарисованная свеча, совпадающая с `last` бакетом API) и, если есть, «замороженный» `forming` — иначе график падает на сырой REST, и «старая» свеча откатывается.
pub fn display_candles_without_live_tick(
    history: &[Vec<f64>],
    sealed: &Option<SealedCandleRow>,
    forming: &Option<FormingBarState>,
    bar_secs: i64,
) -> Vec<Vec<f64>> {
    let last_bar_t = max_aligned_bar_time(history, bar_secs).unwrap_or(i64::MIN);
    let mut out = history.to_vec();
    if let Some((st, row)) = sealed {
        if *st == last_bar_t {
            if let Some(j) = out.iter().rposition(|r| {
                r.len() >= 5 && bar_time_for_timestamp(r[0] as i64, bar_secs) == *st
            }) {
                out[j] = row.clone();
            }
        }
    }
    if let Some(f) = forming {
        if f.bar_t > last_bar_t {
            out.push(vec![
                f.bar_t as f64,
                encode_chainlink_price(f.open),
                encode_chainlink_price(f.high),
                encode_chainlink_price(f.low),
                encode_chainlink_price(f.close),
            ]);
        }
    }
    out
}

/// Копия `candles` с обновлённой **последним по бакету** (как `barTimeForTimestamp`) свечой: close = `live`…
pub fn candles_with_live_last(candles: &[Vec<f64>], live: f64, bar_s: i64) -> Vec<Vec<f64>> {
    if candles.is_empty() {
        return vec![];
    }
    let Some(idx) = rightmost_in_highest_bar_bucket(candles, bar_s) else {
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

/// `stroke` той же свечи, что справа: самый новый `bar` на оси (как в `box_plot_from_history`).
pub fn last_candle_stroke_color(candles: &[Vec<f64>], bar_s: i64) -> Option<Color32> {
    let idx = rightmost_in_highest_bar_bucket(candles, bar_s)?;
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
    let bar_s = bar_secs as i64;
    // Ключ = начало бакета, как в `barTimeForTimestamp` — иначе два t в одном интервале из-за
    // f64/JSON съезжают в разные i64 и в HashMap «теряется» свеча; соседние сливались в один x.
    let mut by_t: HashMap<i64, (f64, f64, f64, f64)> = HashMap::new();

    for row in candles {
        if row.len() < 5 {
            continue;
        }
        let t = bar_time_for_timestamp(row[0] as i64, bar_s);
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

    Some(BoxPlot::new("OHLC", boxes).vertical())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(t: i64, o: f64, h: f64, l: f64, c: f64) -> Vec<f64> {
        vec![
            t as f64,
            encode_chainlink_price(o),
            encode_chainlink_price(h),
            encode_chainlink_price(l),
            encode_chainlink_price(c),
        ]
    }

    #[test]
    fn merge_appends_new_bar_when_tick_in_next_bucket() {
        let history = vec![row(1000, 10.0, 11.0, 9.0, 10.5)];
        let mut forming = None;
        let mut sealed = None;
        // 5m bucket: last_t=1000; t=1600 -> bar=1500 > 1000
        let bar_secs = 300;
        let out = merge_history_with_live(
            &history,
            10.2,
            1600,
            bar_secs,
            &mut forming,
            &mut sealed,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[1][0] as i64, 1500);
        assert!(forming.is_some());
    }

    #[test]
    fn merge_updates_last_row_when_tick_in_same_bucket_as_api() {
        let history = vec![row(1500, 10.0, 11.0, 9.0, 10.5)];
        let mut forming = Some(FormingBarState {
            bar_t: 9999,
            open: 1.0,
            high: 1.0,
            low: 1.0,
            close: 1.0,
        });
        let mut sealed = None;
        let out = merge_history_with_live(&history, 10.8, 1600, 300, &mut forming, &mut sealed);
        assert_eq!(out.len(), 1);
        assert!(forming.is_none());
        assert!((decode_chainlink_price(out[0][4]) - 10.8).abs() < 1e-9);
        assert_eq!(sealed.map(|(t, _)| t), Some(1500));
    }

    #[test]
    fn display_without_tick_keeps_sealed_not_raw_rest() {
        let history = vec![row(1500, 10.0, 11.0, 9.0, 9.0)];
        let sealed = Some((
            1500_i64,
            {
                let mut c = history[0].clone();
                c[4] = encode_chainlink_price(10.5);
                c[2] = encode_chainlink_price(11.0);
                c[3] = encode_chainlink_price(9.0);
                c
            },
        ));
        let out = display_candles_without_live_tick(
            &history,
            &sealed,
            &None,
            300,
        );
        assert!((decode_chainlink_price(out[0][4]) - 10.5).abs() < 1e-9);
    }

    #[test]
    fn merge_uses_sealed_for_previous_bar_when_new_bucket_appends() {
        // REST ещё с last close=9, live дорисовывает до 10.5, затем новый бакет
        let history = vec![row(1500, 10.0, 11.0, 9.0, 9.0)];
        let mut sealed = None;
        let mut forming = None;
        merge_history_with_live(&history, 10.5, 1600, 300, &mut forming, &mut sealed);
        let out = merge_history_with_live(&history, 10.1, 1900, 300, &mut forming, &mut sealed);
        assert_eq!(out.len(), 2);
        assert!(
            (decode_chainlink_price(out[0][4]) - 10.5).abs() < 1e-6,
            "квант REST не откатывает закрытие: {}",
            decode_chainlink_price(out[0][4])
        );
        assert!((decode_chainlink_price(out[1][4]) - 10.1).abs() < 1e-9);
    }

    #[test]
    fn merge_accepts_stream_time_in_millis_aligns_with_history_sec() {
        // API — сек; стрим — мс (тот же момент). Без нормализации сравнение `bar` с историей ломается.
        let t_last = 1_700_000_400i64;
        let history = vec![row(1_700_000_100, 10.0, 11.0, 9.0, 10.0), row(t_last, 10.1, 10.2, 10.0, 10.15)];
        let mut forming = None;
        let mut sealed = None;
        let bar_secs = 300;
        let t_ms = t_last * 1000 + 50;
        let out = merge_history_with_live(
            &history,
            10.22,
            t_ms,
            bar_secs,
            &mut forming,
            &mut sealed,
        );
        assert_eq!(out.len(), 2, "все ряды API на месте");
        assert!((decode_chainlink_price(out[1][4]) - 10.22).abs() < 1e-6);
    }
}
