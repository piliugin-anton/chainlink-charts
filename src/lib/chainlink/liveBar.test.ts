import type { UTCTimestamp } from "lightweight-charts";
import { describe, expect, it } from "vitest";
import { computeFormingBar } from "./liveBar";

/** 5m buckets: use times aligned to 300s for predictable barTimeForTimestamp. */
const RES_5M = "5m";

describe("computeFormingBar", () => {
  it("accumulates ticks in a new bucket not yet in history (raw OHLC)", () => {
    const lastHistoryT = 1500 as UTCTimestamp; // last API bar
    const h = [
      { time: lastHistoryT, open: 100, high: 110, low: 90, close: 100 },
    ];
    // t=2000 → bar = floor(2000/300)*300 = 1800 > 1500
    const r0 = computeFormingBar(h, { price: 101, t: 2000 }, RES_5M, null);
    expect(r0).not.toBeNull();
    expect(Number(r0!.time)).toBe(1800);
    expect(r0!.open).toBe(101);
    expect(r0!.high).toBe(101);
    expect(r0!.low).toBe(101);
    expect(r0!.close).toBe(101);

    const r1 = computeFormingBar(h, { price: 104, t: 2010 }, RES_5M, r0);
    expect(r1!.open).toBe(101);
    expect(r1!.close).toBe(104);
    expect(r1!.high).toBe(104);
    expect(r1!.low).toBe(101);
  });

  it("merges with the last history row when bar time matches that row", () => {
    const tRow = 1800 as UTCTimestamp;
    const h = [
      { time: tRow, open: 50, high: 60, low: 40, close: 55 },
    ];
    // t=1950 → bar = floor(1950/300)*300 = 1800 === lastT
    const r = computeFormingBar(h, { price: 58, t: 1950 }, RES_5M, null);
    expect(r).not.toBeNull();
    expect(r!.open).toBe(50);
    expect(r!.close).toBe(58);
    expect(r!.high).toBe(60);
    expect(r!.low).toBe(40);
  });

  it("preserves live-tick extremes across multiple ticks within the last history bucket", () => {
    const tRow = 1800 as UTCTimestamp;
    const h = [{ time: tRow, open: 50, high: 60, low: 40, close: 55 }];

    // tick 1: price 70 > history high 60 → new high = 70
    const r0 = computeFormingBar(h, { price: 70, t: 1900 }, RES_5M, null);
    expect(r0!.high).toBe(70);
    expect(r0!.low).toBe(40);

    // tick 2: price 55 — should NOT lose the 70 high established by tick 1
    const r1 = computeFormingBar(h, { price: 55, t: 1920 }, RES_5M, r0);
    expect(r1!.high).toBe(70);

    // tick 3: price 30 < history low 40 → new low = 30
    const r2 = computeFormingBar(h, { price: 30, t: 1940 }, RES_5M, r1);
    expect(r2!.low).toBe(30);

    // tick 4: price 55 — should NOT lose the 30 low established by tick 3
    const r3 = computeFormingBar(h, { price: 55, t: 1960 }, RES_5M, r2);
    expect(r3!.low).toBe(30);
  });
});
