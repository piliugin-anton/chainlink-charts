import { NextResponse } from "next/server";

import { getAccessToken } from "@/lib/chainlink/auth";
import { getChainlinkConfig, isChainlinkConfigured } from "@/lib/chainlink/env";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

/**
 * Dev helper: proxy `symbol_info` to confirm feed names (BTCUSD, etc.).
 */
export async function GET() {
  if (!isChainlinkConfigured()) {
    return NextResponse.json(
      { error: "Server is not configured for Chainlink (missing env vars)." },
      { status: 503 }
    );
  }

  try {
    const { baseUrl } = getChainlinkConfig();
    const token = await getAccessToken();
    const res = await fetch(`${baseUrl}/api/v1/symbol_info`, {
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
    console.error("[symbols]", e);
    return NextResponse.json(
      { error: e instanceof Error ? e.message : "Upstream error" },
      { status: 500 }
    );
  }
}
