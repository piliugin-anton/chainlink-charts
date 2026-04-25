import { NextRequest, NextResponse } from "next/server";

import { getAccessToken } from "@/lib/chainlink/auth";
import { isAllowedSymbol } from "@/lib/chainlink/constants";
import { getChainlinkConfig, isChainlinkConfigured } from "@/lib/chainlink/env";
import { historyQuerySchema } from "@/lib/chainlink/schemas";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(req: NextRequest) {
  if (!isChainlinkConfigured()) {
    return NextResponse.json(
      { error: "Server is not configured for Chainlink (missing env vars)." },
      { status: 503 }
    );
  }

  const sp = req.nextUrl.searchParams;
  const parsed = historyQuerySchema.safeParse({
    symbol: sp.get("symbol"),
    resolution: sp.get("resolution") ?? "1h",
    from: sp.get("from"),
    to: sp.get("to"),
  });

  if (!parsed.success) {
    return NextResponse.json(
      { error: "Invalid query", details: parsed.error.flatten() },
      { status: 400 }
    );
  }

  const { symbol, resolution, from, to } = parsed.data;
  if (!isAllowedSymbol(symbol)) {
    return NextResponse.json({ error: "Symbol not allowed" }, { status: 400 });
  }

  if (from >= to) {
    return NextResponse.json(
      { error: "`from` must be less than `to`" },
      { status: 400 }
    );
  }

  try {
    const { baseUrl } = getChainlinkConfig();
    const token = await getAccessToken();
    const url = new URL(`${baseUrl}/api/v1/history/rows`);
    url.searchParams.set("symbol", symbol);
    url.searchParams.set("resolution", resolution);
    url.searchParams.set("from", String(from));
    url.searchParams.set("to", String(to));

    const res = await fetch(url.toString(), {
      headers: { Authorization: `Bearer ${token}` },
      cache: "no-store",
    });

    const body = await res.text();
    return new NextResponse(body, {
      status: res.status,
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "no-store",
      },
    });
  } catch (e) {
    console.error("[history]", e);
    return NextResponse.json(
      { error: e instanceof Error ? e.message : "Upstream error" },
      { status: 500 }
    );
  }
}
