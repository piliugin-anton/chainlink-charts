"use client";

import {
  CandlestickSeries,
  ColorType,
  createChart,
  type CandlestickData,
  type IChartApi,
  type ISeriesApi,
} from "lightweight-charts";
import { useEffect, useLayoutEffect, useRef } from "react";

type Props = {
  data: CandlestickData[];
  liveBar: CandlestickData | null;
};

export function PriceChart({ data, liveBar }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) {
      return;
    }

    const chart = createChart(el, {
      layout: {
        background: { type: ColorType.Solid, color: "#09090b" },
        textColor: "#a1a1aa",
      },
      grid: {
        vertLines: { color: "#27272a" },
        horzLines: { color: "#27272a" },
      },
      width: el.clientWidth,
      height: 420,
      timeScale: { timeVisible: true, secondsVisible: false },
    });

    const series = chart.addSeries(CandlestickSeries, {
      upColor: "#22c55e",
      downColor: "#ef4444",
      borderVisible: false,
      wickUpColor: "#22c55e",
      wickDownColor: "#ef4444",
    });

    chartRef.current = chart;
    seriesRef.current = series;

    const ro = new ResizeObserver(() => {
      if (!wrap.current) {
        return;
      }
      chart.applyOptions({ width: wrap.current.clientWidth });
    });
    ro.observe(el);

    return () => {
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, []);

  useEffect(() => {
    const s = seriesRef.current;
    if (!s) {
      return;
    }
    s.setData(data);
  }, [data]);

  useEffect(() => {
    const s = seriesRef.current;
    if (!s || !liveBar) {
      return;
    }
    s.update(liveBar);
  }, [liveBar]);

  return <div className="h-[420px] w-full min-w-0" ref={wrap} />;
}
