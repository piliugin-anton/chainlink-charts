/**
 * Parse resolution strings from the Candlestick API
 * (e.g. 1m, 5m, 24h, 1d, 1w, 6M, 1y). Used for bar alignment on the client.
 */
const RESOLUTION_RE = /^(\d+)(m|h|d|w|M|y)$/;

export function resolutionToSeconds(resolution: string): number | null {
  const m = resolution.trim().match(RESOLUTION_RE);
  if (!m) return null;
  const n = Number(m[1]);
  if (!Number.isFinite(n) || n <= 0) return null;
  const u = m[2];
  switch (u) {
    case "m":
      return n * 60;
    case "h":
      return n * 3600;
    case "d":
      return n * 86400;
    case "w":
      return n * 604800;
    case "M":
      return n * 2592000; // ~30d — bar alignment only
    case "y":
      return n * 31536000; // ~365d
    default:
      return null;
  }
}

/**
 * Coerce user resolution to satisfy Chainlink supported ranges vs window size.
 * @see https://docs.chain.link/data-streams/reference/candlestick-api#supported-resolutions
 */
export function coerceResolutionForWindow(
  resolution: string,
  windowSec: number
): string {
  const sec = resolutionToSeconds(resolution);
  if (sec === null) return "1h";

  // 1 min – 24 h window: 1m – 24h
  if (windowSec <= 86400) {
    if (sec < 60) return "1m";
    if (sec > 86400) return "24h";
    return resolution;
  }

  // 1 – 5 days: 5m – 5d
  if (windowSec <= 5 * 86400) {
    const min = 5 * 60;
    const max = 5 * 86400;
    return clampResolution(sec, min, max, resolution);
  }

  // 5 – 30 days: 30m – 30d
  if (windowSec <= 30 * 86400) {
    const min = 30 * 60;
    const max = 30 * 86400;
    return clampResolution(sec, min, max, resolution);
  }

  // 30 – 90 days: 1h – 90d
  if (windowSec <= 90 * 86400) {
    const min = 3600;
    const max = 90 * 86400;
    return clampResolution(sec, min, max, resolution);
  }

  // wider windows: prefer coarser bars
  if (sec < 3600) return "1h";
  return resolution;
}

function clampResolution(
  sec: number,
  minSec: number,
  maxSec: number,
  fallback: string
): string {
  if (sec >= minSec && sec <= maxSec) return fallback;
  if (sec < minSec) {
    if (minSec === 60) return "1m";
    if (minSec === 5 * 60) return "5m";
    if (minSec === 30 * 60) return "30m";
    if (minSec === 3600) return "1h";
  }
  if (sec > maxSec) {
    if (maxSec === 86400) return "24h";
    if (maxSec === 5 * 86400) return "5d";
  }
  return "1h";
}
