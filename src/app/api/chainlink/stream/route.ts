import { NextResponse } from "next/server";

import { getAccessToken } from "@/lib/chainlink/auth";
import { STREAMING_SYMBOLS_PARAM } from "@/lib/chainlink/constants";
import { getChainlinkConfig, isChainlinkConfigured } from "@/lib/chainlink/env";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export const maxDuration = 300;

export async function GET(request: Request) {
  if (!isChainlinkConfigured()) {
    return NextResponse.json(
      { error: "Server is not configured for Chainlink (missing env vars)." },
      { status: 503 }
    );
  }

  try {
    const { baseUrl } = getChainlinkConfig();
    const token = await getAccessToken();
    const q = new URLSearchParams();
    q.set("symbol", STREAMING_SYMBOLS_PARAM);
    const url = `${baseUrl}/api/v1/streaming?${q.toString()}`;

    const upstream = await fetch(url, {
      signal: request.signal,
      headers: {
        Authorization: `Bearer ${token}`,
        Connection: "keep-alive",
      },
      cache: "no-store",
    });

    if (!upstream.ok) {
      const t = await upstream.text().catch(() => "");
      return NextResponse.json(
        { error: "Upstream stream error", status: upstream.status, body: t.slice(0, 500) },
        { status: 502 }
      );
    }

    if (!upstream.body) {
      return NextResponse.json(
        { error: "No response body from upstream" },
        { status: 502 }
      );
    }

    const ct = upstream.headers.get("content-type") ?? "application/json";

    return new NextResponse(upstream.body, {
      status: 200,
      headers: {
        "Cache-Control": "no-store, no-transform",
        "X-Accel-Buffering": "no",
        Connection: "keep-alive",
        "Content-Type": ct,
      },
    });
  } catch (e) {
    if (e instanceof Error && e.name === "AbortError") {
      return new NextResponse(null, { status: 204 });
    }
    console.error("[stream]", e);
    return NextResponse.json(
      { error: e instanceof Error ? e.message : "Stream error" },
      { status: 500 }
    );
  }
}
