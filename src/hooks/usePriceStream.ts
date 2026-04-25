"use client";

import { useEffect, useRef, useState } from "react";

import { decodeChainlinkPrice } from "@/lib/chainlink/price";
import { feedJsonChunks } from "@/lib/stream/ndjson";

export type StreamStatus =
  | "idle"
  | "connecting"
  | "live"
  | "reconnecting"
  | "error"
  | "unconfigured";

export type LastPrice = { price: number; t: number };

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Long-lived fetch to `/api/chainlink/stream`, parses JSON trade messages.
 */
export function usePriceStream(enabled: boolean) {
  const [prices, setPrices] = useState<Record<string, LastPrice>>({});
  const [status, setStatus] = useState<StreamStatus>("idle");
  const [lastError, setLastError] = useState<string | null>(null);

  const countRef = useRef(0);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const ac = new AbortController();
    let buf = "";
    let backoff = 1000;
    const id = ++countRef.current;

    const run = async () => {
      while (!ac.signal.aborted && countRef.current === id) {
        setStatus((s) => (s === "live" ? "reconnecting" : "connecting"));
        setLastError(null);
        try {
          const res = await fetch("/api/chainlink/stream", {
            signal: ac.signal,
            cache: "no-store",
          });

          if (res.status === 503) {
            setStatus("unconfigured");
            return;
          }

          if (!res.ok || !res.body) {
            const t = await res.text().catch(() => "");
            throw new Error(`HTTP ${res.status} ${t.slice(0, 200)}`);
          }

          setStatus("live");
          backoff = 1000;
          const reader = res.body.getReader();
          const dec = new TextDecoder();
          buf = "";

          while (!ac.signal.aborted && countRef.current === id) {
            const { value, done } = await reader.read();
            if (done) {
              break;
            }
            const chunk = dec.decode(value, { stream: true });
            const { buffer, messages } = feedJsonChunks(buf, chunk);
            buf = buffer;
            for (const msg of messages) {
              if (typeof msg !== "object" || !msg) continue;
              if ("heartbeat" in msg) continue;
              const m = msg as { f?: string; i?: string; p?: number; t?: number };
              if (m.f === "t" && m.i && typeof m.p === "number" && typeof m.t === "number") {
                const price = decodeChainlinkPrice(m.p);
                const t = m.t;
                setPrices((prev) => ({ ...prev, [m.i!]: { price, t } }));
              }
            }
          }
        } catch (e) {
          if (ac.signal.aborted) return;
          setStatus("error");
          setLastError(e instanceof Error ? e.message : "Stream error");
          await sleep(backoff);
          backoff = Math.min(backoff * 2, 30_000);
        }
      }
    };

    void run();
    return () => {
      ac.abort();
    };
  }, [enabled]);

  return {
    prices,
    status: enabled ? status : "idle",
    lastError: enabled ? lastError : null,
  };
}
