/**
 * Chainlink candlestick & stream samples return prices as large floats
 * (fixed-point, effectively scaled by 1e18 in practice). Use the same
 * path for all displayed numbers and for chart series.
 */
export const PRICE_SCALE = 1e18;

export function decodeChainlinkPrice(raw: number): number {
  if (!Number.isFinite(raw) || raw === 0) return 0;
  return raw / PRICE_SCALE;
}
