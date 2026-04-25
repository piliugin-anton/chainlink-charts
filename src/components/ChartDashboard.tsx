"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";

import { usePriceStream } from "@/hooks/usePriceStream";
import { coerceResolutionForWindow } from "@/lib/chainlink/resolution";
import {
  type HistoryRowsResponse,
  mapHistoryToChartData,
} from "@/lib/chainlink/candles";
import { computeFormingBar } from "@/lib/chainlink/liveBar";
import { ASSET_LIST, type AssetKey, API_SYMBOLS } from "@/lib/chainlink/constants";
import { PriceChart } from "./PriceChart";

const RANGE_OPTIONS = [
  { id: "24h" as const, label: "24h", seconds: 86400 },
  { id: "7d" as const, label: "7d", seconds: 7 * 86400 },
  { id: "30d" as const, label: "30d", seconds: 30 * 86400 },
];

const RESOLUTION_PRESETS: Record<(typeof RANGE_OPTIONS)[number]["id"], string[]> = {
  "24h": ["1m", "5m", "15m", "1h", "4h", "1d"],
  "7d": ["5m", "15m", "1h", "4h", "1d", "1w"],
  "30d": ["1h", "4h", "1d", "1w", "1M"],
};

const DEFAULT_RESOLUTION: Record<(typeof RANGE_OPTIONS)[number]["id"], string> = {
  "24h": "5m",
  "7d": "1h",
  "30d": "4h",
};

export function ChartDashboard() {
  const qc = useQueryClient();
  const [asset, setAsset] = useState<AssetKey>("BTC");
  const [rangeId, setRangeId] = useState<(typeof RANGE_OPTIONS)[number]["id"]>("24h");
  const [resolution, setResolution] = useState(() => DEFAULT_RESOLUTION["24h"]);

  const rangeSec = RANGE_OPTIONS.find((r) => r.id === rangeId)!.seconds;
  const apiSymbol = API_SYMBOLS[asset];

  const coercedResolution = useMemo(
    () => coerceResolutionForWindow(resolution, rangeSec),
    [resolution, rangeSec]
  );

  const { prices, status, lastError } = usePriceStream(true);

  const prevStatus = useRef(status);
  useEffect(() => {
    if (prevStatus.current === "reconnecting" && status === "live") {
      void qc.invalidateQueries({ queryKey: ["candles"] });
    }
    prevStatus.current = status;
  }, [status, qc]);

  const historyQuery = useQuery({
    queryKey: ["candles", apiSymbol, coercedResolution, rangeId],
    queryFn: async (): Promise<HistoryRowsResponse> => {
      const now = Math.floor(Date.now() / 1000);
      const from = now - rangeSec;
      const u = new URL(
        "/api/chainlink/history",
        typeof window !== "undefined" ? window.location.origin : "http://localhost:3000"
      );
      u.searchParams.set("symbol", apiSymbol);
      u.searchParams.set("resolution", coercedResolution);
      u.searchParams.set("from", String(from));
      u.searchParams.set("to", String(now));
      const r = await fetch(u);
      if (!r.ok) {
        const t = await r.text();
        let msg = t;
        try {
          const j = JSON.parse(t) as { error?: string };
          if (j.error) {
            msg = j.error;
          }
        } catch {
          /* keep text */
        }
        throw new Error(msg);
      }
      return r.json() as Promise<HistoryRowsResponse>;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
  });

  const chartData = useMemo(
    () =>
      historyQuery.data
        ? mapHistoryToChartData(historyQuery.data)
        : [],
    [historyQuery.data]
  );

  const lastTick = prices[apiSymbol];

  const liveBar = useMemo(
    () => computeFormingBar(chartData, lastTick, coercedResolution),
    [chartData, lastTick, coercedResolution]
  );

  const unconfigured = status === "unconfigured";
  const streamLabel =
    status === "live"
      ? "Live"
      : status === "connecting" || status === "reconnecting"
        ? "Connecting…"
        : status === "unconfigured"
          ? "Not configured"
          : status === "error"
            ? "Stream error"
            : "—";

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6 px-4 py-8">
      <header className="flex flex-col gap-1 border-b border-zinc-800 pb-6">
        <h1 className="text-2xl font-semibold tracking-tight text-zinc-100">
          Chainlink live charts
        </h1>
        <p className="text-sm text-zinc-500">
          Data Streams Candlestick API — BTC, ETH, SOL, XRP
        </p>
      </header>

      {unconfigured && (
        <div
          className="rounded-lg border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-200"
          role="status"
        >
          Set <code className="font-mono text-amber-100">CHAINLINK_BASE_URL</code>,{" "}
          <code className="font-mono text-amber-100">CHAINLINK_USER_ID</code>, and{" "}
          <code className="font-mono text-amber-100">CHAINLINK_API_KEY</code> in{" "}
          <code className="font-mono">.env.local</code>, then restart <code>npm run dev</code>
          . See <span className="font-mono">.env.example</span> and README.
        </div>
      )}

      {historyQuery.isError && (
        <div
          className="rounded-lg border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-200"
          role="alert"
        >
          History: {historyQuery.error instanceof Error ? historyQuery.error.message : "Error"}
        </div>
      )}

      <section
        className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4"
        aria-label="Last prices"
      >
        {ASSET_LIST.map((a) => {
          const tick = prices[a.apiSymbol];
          return (
            <button
              key={a.key}
              type="button"
              onClick={() => setAsset(a.key)}
              className={
                a.key === asset
                  ? "rounded-xl border border-emerald-500/50 bg-zinc-900/80 p-4 text-left ring-1 ring-emerald-500/20"
                  : "rounded-xl border border-zinc-800 bg-zinc-950/60 p-4 text-left hover:border-zinc-700"
              }
            >
              <div className="text-xs font-medium uppercase tracking-wider text-zinc-500">
                {a.label}
              </div>
              <div className="mt-1 font-mono text-lg text-zinc-100">
                {tick
                  ? <>$ {tick.price.toLocaleString("en-US", {
                      minimumFractionDigits: 2,
                      maximumFractionDigits: 2,
                    })}</>
                  : "—"}
              </div>
            </button>
          );
        })}
      </section>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div className="flex flex-wrap gap-2" role="group" aria-label="Range">
          {RANGE_OPTIONS.map((r) => (
            <button
              key={r.id}
              type="button"
              onClick={() => {
                setRangeId(r.id);
                setResolution(DEFAULT_RESOLUTION[r.id]);
              }}
              className={
                rangeId === r.id
                  ? "rounded-md bg-zinc-100 px-3 py-1.5 text-sm font-medium text-zinc-900"
                  : "rounded-md bg-zinc-800/80 px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800"
              }
            >
              {r.label}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-zinc-500">Resolution</span>
          <select
            value={RESOLUTION_PRESETS[rangeId].includes(resolution) ? resolution : DEFAULT_RESOLUTION[rangeId]}
            onChange={(e) => setResolution(e.target.value)}
            className="rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1.5 text-sm text-zinc-200"
          >
            {RESOLUTION_PRESETS[rangeId].map((res) => (
              <option key={res} value={res}>
                {res}
              </option>
            ))}
          </select>
          {coercedResolution !== resolution && (
            <span className="text-xs text-amber-400" title="Adjusted for API window rules">
              API: {coercedResolution}
            </span>
          )}
        </div>
      </div>

      <div className="flex items-center justify-between text-xs text-zinc-500">
        <span>
          Chart: {ASSET_LIST.find((a) => a.key === asset)?.label} · {coercedResolution}
        </span>
        <span>
          Stream: {streamLabel}
          {lastError ? ` (${lastError})` : ""}
        </span>
      </div>

      <div className="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-950 p-1">
        {historyQuery.isLoading ? (
          <div className="flex h-[420px] items-center justify-center text-sm text-zinc-500">
            Loading candles…
          </div>
        ) : chartData.length === 0 && !historyQuery.isError ? (
          <div className="flex h-[420px] flex-col items-center justify-center gap-2 text-sm text-zinc-500">
            <p>No candle data in this range.</p>
            <p className="text-xs">Try a coarser resolution or a wider range.</p>
          </div>
        ) : (
          <PriceChart data={chartData} liveBar={liveBar} />
        )}
      </div>
    </div>
  );
}
