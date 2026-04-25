import type { UTCTimestamp } from "lightweight-charts";
import { describe, expect, it } from "vitest";
import { normalizeCandleForDisplay } from "./candles";

describe("normalizeCandleForDisplay", () => {
  it("gives a non-zero range and body when all OHLC are equal (new / single-tick bar)", () => {
    const t = 1700 as UTCTimestamp;
    const c = normalizeCandleForDisplay({
      time: t,
      open: 50_000,
      high: 50_000,
      low: 50_000,
      close: 50_000,
    });
    expect(c.high).toBeGreaterThan(c.low);
    expect(c.open).toBeLessThan(c.close);
    expect(c.open).toBeGreaterThanOrEqual(c.low);
    expect(c.close).toBeLessThanOrEqual(c.high);
  });
});
