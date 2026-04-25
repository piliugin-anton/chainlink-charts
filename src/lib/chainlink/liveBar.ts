import type { CandlestickData, UTCTimestamp } from "lightweight-charts";

import { barTimeForTimestamp } from "./candles";

/**
 * Forming bar from the latest tick, merged with the last historical candle when times match.
 */
export function computeFormingBar(
  historyAsc: CandlestickData[],
  tick: { price: number; t: number } | undefined,
  resolution: string
): CandlestickData | null {
  if (!tick) {
    return null;
  }
  const bar = barTimeForTimestamp(tick.t, resolution);
  const p = tick.price;
  const barT = bar as UTCTimestamp;
  const last = historyAsc[historyAsc.length - 1];

  if (!last) {
    return { time: barT, open: p, high: p, low: p, close: p };
  }

  const lastT = Number(last.time);

  if (bar > lastT) {
    return { time: barT, open: p, high: p, low: p, close: p };
  }

  if (bar < lastT) {
    return null;
  }

  return {
    time: barT,
    open: last.open,
    high: Math.max(last.high, p),
    low: Math.min(last.low, p),
    close: p,
  };
}
