import type { CandlestickData, UTCTimestamp } from "lightweight-charts";

import { decodeChainlinkPrice } from "./price";
import { resolutionToSeconds } from "./resolution";

export type HistoryRowsResponse = {
  s?: string;
  error?: string;
  candles: [number, number, number, number, number, number][];
};

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
  // Deduplicate by time (keeps last)
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
