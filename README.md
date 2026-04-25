# Chainlink live charts

Next.js (App Router) dashboard for **BTC, ETH, SOL, and XRP** using the [Chainlink Data Streams Candlestick API](https://docs.chain.link/data-streams/reference/candlestick-api). API credentials stay on the server; the browser only calls same-origin BFF routes.

## Setup

1. Copy environment variables:

   ```bash
   cp .env.example .env.local
   ```

2. Fill in values from Chainlink (testnet or mainnet `priceapi` host — see [documentation](https://docs.chain.link/data-streams/reference/candlestick-api#domains)):

   - `CHAINLINK_BASE_URL` — e.g. `https://priceapi.testnet-dataengine.chain.link`
   - `CHAINLINK_USER_ID` — login for `/api/v1/authorize`
   - `CHAINLINK_API_KEY` — password / API key

3. Install and run:

   ```bash
   npm install
   npm run dev
   ```

4. Open [http://localhost:3000](http://localhost:3000).

If env vars are missing, the UI shows a banner and history/stream endpoints return HTTP 503.

## Endpoints (BFF)

| Route | Purpose |
|-------|---------|
| `GET /api/chainlink/history` | Proxies `history/rows` (query: `symbol`, `resolution`, `from`, `to`) |
| `GET /api/chainlink/stream` | Proxies multi-symbol `streaming` for allowlisted pairs |
| `GET /api/chainlink/symbols` | Proxies `symbol_info` (optional dev check) |

## Smoke checklist (with real credentials)

- [ ] `/api/chainlink/symbols` returns 200 and lists expected symbols (e.g. `BTCUSD`).
- [ ] Main chart loads candles after picking range/resolution.
- [ ] Price tiles update when stream is live (status “Live”).
- [ ] After a simulated network drop, stream reconnects and candles can be refreshed (auto refetch on reconnect).

## Production notes

- Use **Node.js runtime** for stream/history routes (`export const runtime = "nodejs"`).
- Long-lived streaming: if you deploy behind nginx, disable buffering for the stream path (e.g. `X-Accel-Buffering: no` is set on the BFF response; your proxy may need `proxy_buffering off`).
- **Vercel:** confirm `maxDuration` and plan limits for long streaming connections.

## Stack

- Next.js 16, React 19, TypeScript
- TanStack Query, Zod, lightweight-charts
- Server-only modules for JWT and env
