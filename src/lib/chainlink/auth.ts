import "server-only";

import { getChainlinkConfig } from "./env";

type TokenCache = { token: string; exp: number };

let cached: TokenCache | null = null;

const SKEW_SEC = 90;

/**
 * JWT for Candlestick API. In-memory only; scales to single serverless instance.
 */
export async function getAccessToken(): Promise<string> {
  const now = Math.floor(Date.now() / 1000);
  if (cached && cached.exp - SKEW_SEC > now) {
    return cached.token;
  }

  const { baseUrl, userId, apiKey } = getChainlinkConfig();
  const res = await fetch(`${baseUrl}/api/v1/authorize`, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ login: userId, password: apiKey }),
    cache: "no-store",
  });

  if (!res.ok) {
    const t = await res.text().catch(() => "");
    throw new Error(`Authorize failed: HTTP ${res.status} ${t.slice(0, 200)}`);
  }

  const data = (await res.json()) as {
    s?: string;
    d?: { access_token?: string; expiration?: number };
  };

  if (data.s !== "ok" || !data.d?.access_token) {
    throw new Error("Invalid authorize response");
  }

  const exp =
    typeof data.d.expiration === "number" && data.d.expiration > now
      ? data.d.expiration
      : now + 3600;

  cached = { token: data.d.access_token, exp };
  return cached.token;
}
