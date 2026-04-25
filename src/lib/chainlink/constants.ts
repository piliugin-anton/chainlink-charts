/** API symbols (must match Chainlink `symbol_info`). */
export const API_SYMBOLS = {
  BTC: "BTCUSD",
  ETH: "ETHUSD",
  SOL: "SOLUSD",
  XRP: "XRPUSD",
} as const;

export type AssetKey = keyof typeof API_SYMBOLS;

export const ASSET_LIST: { key: AssetKey; label: string; apiSymbol: string }[] = [
  { key: "BTC", label: "Bitcoin", apiSymbol: API_SYMBOLS.BTC },
  { key: "ETH", label: "Ethereum", apiSymbol: API_SYMBOLS.ETH },
  { key: "SOL", label: "Solana", apiSymbol: API_SYMBOLS.SOL },
  { key: "XRP", label: "XRP", apiSymbol: API_SYMBOLS.XRP },
];

export const ALLOWED_API_SYMBOLS: ReadonlySet<string> = new Set(
  ASSET_LIST.map((a) => a.apiSymbol)
);

export function isAllowedSymbol(symbol: string): boolean {
  return ALLOWED_API_SYMBOLS.has(symbol);
}

/** Default streaming query (comma-separated, order stable). */
export const STREAMING_SYMBOLS_PARAM = ASSET_LIST.map((a) => a.apiSymbol).join(
  ","
);
