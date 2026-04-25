import type { CandlestickData, UTCTimestamp } from "lightweight-charts";

import { barTimeForTimestamp } from "./candles";

/**
 * Forming bar from the latest tick and history, optionally continuing a not-yet-in-API bar.
 * Returns raw OHLC; apply `normalizeCandleForDisplay` in the UI for the chart.
 *
 * @param previousFormingRaw - When the current bar time is greater than the last history time,
 *   the bucket is not in the API array yet. Pass the **raw** result from the previous call for
 *   the same `time` to accumulate high/low/close and keep a stable `open` across ticks.
 */
export function computeFormingBar(
  historyAsc: CandlestickData[],
  tick: { price: number; t: number } | undefined,
  resolution: string,
  previousFormingRaw: CandlestickData | null = null
): CandlestickData | null {
  if (!tick) {
    return null;
  }
  const bar = barTimeForTimestamp(tick.t, resolution);
  const p = tick.price;
  const barT = bar as UTCTimestamp;
  const last = historyAsc[historyAsc.length - 1];
  const lastT = last ? Number(last.time) : -Infinity;
  const prevT = previousFormingRaw ? Number(previousFormingRaw.time) : NaN;

  if (bar < lastT) {
    return null;
  }

  if (bar === lastT) {
    return {
      time: barT,
      open: last!.open,
      high: Math.max(last!.high, p),
      low: Math.min(last!.low, p),
      close: p,
    };
  }

  // bar > lastT: new bucket not in history, or no history
  if (previousFormingRaw && prevT === bar) {
    return {
      time: barT,
      open: previousFormingRaw.open,
      high: Math.max(previousFormingRaw.high, p),
      low: Math.min(previousFormingRaw.low, p),
      close: p,
    };
  }
  return { time: barT, open: p, high: p, low: p, close: p };
}
