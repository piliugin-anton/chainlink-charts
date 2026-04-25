import type { CandlestickData, UTCTimestamp } from "lightweight-charts";

import { decodeChainlinkPrice } from "./price";
import { resolutionToSeconds } from "./resolution";

export type HistoryRowsResponse = {
  s?: string;
  error?: string;
  candles: [number, number, number, number, number, number][];
};

/** Min relative spread so flat OHLC (e.g. a new live bar) still has a visible wick in the chart. */
function minPriceSpread(anchor: number): number {
  // 1e-8×price was ~0.0007 on ~77k USD — subpixel after autoscale. ~1e-5×price is chart-visible;
  // floor keeps tiny quotes readable.
  return Math.max(Math.abs(anchor) * 1e-5, 0.01);
}

/**
 * Decompresses zero-range high/low and zero-height bodies so lightweight-charts does not
 * render a 1px line (common when a bar has only one tick: open=high=low=close).
 */
export function normalizeCandleForDisplay(c: CandlestickData): CandlestickData {
  const { time } = c;
  let { open, high, low, close } = c;

  const ref = (high + low) / 2 || close;
  const spread = minPriceSpread(ref);
  if (Math.abs(high - low) < spread) {
    const m = (high + low) / 2;
    const half = spread / 2;
    high = m + half;
    low = m - half;
  }

  const range = high - low;
  if (range > 0 && Math.abs(open - close) < minPriceSpread((open + close) / 2)) {
    const m = (open + close) / 2;
    const bodyHalf = Math.min(
      Math.max(minPriceSpread(m) * 0.2, range * 0.02),
      range * 0.45
    );
    if (2 * bodyHalf < range) {
      open = m - bodyHalf;
      close = m + bodyHalf;
    }
  }

  return { time, open, high, low, close };
}

export function mapHistoryToChartData(
  res: HistoryRowsResponse
): CandlestickData[] {
  const rows = res.candles ?? [];
  const out: CandlestickData[] = rows.map((c) => ({
    time: c[0] as UTCTimestamp,
    open: decodeChainlinkPrice(c[1]),
    high: decodeChainlinkPrice(c[2]),
    low: decodeChainlinkPrice(c[3]),
    close: decodeChainlinkPrice(c[4]),
  }));
  out.sort((a, b) => Number(a.time) - Number(b.time));
  // Deduplicate by time (keeps last). Raw OHLC only — do not mix display nudges with merge logic.
  const byT = new Map<number, CandlestickData>();
  for (const c of out) {
    byT.set(Number(c.time), c);
  }
  return [...byT.values()].sort((a, b) => Number(a.time) - Number(b.time));
}

export function barTimeForTimestamp(tsSec: number, resolution: string): number {
  const sec = resolutionToSeconds(resolution) ?? 3600;
  return Math.floor(tsSec / sec) * sec;
}
