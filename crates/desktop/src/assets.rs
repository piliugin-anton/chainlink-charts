//! Static asset list (mirrors `src/lib/chainlink/constants.ts`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetRow {
    pub key: &'static str,
    pub label: &'static str,
    pub api_symbol: &'static str,
    pub feed_id: &'static str,
}

/// Chainlink Data Streams feed IDs for WebSocket streaming.
/// BTC and ETH use testnet IDs (Arbitrum Sepolia) — replace with mainnet IDs from data.chain.link/streams.
/// SOL and XRP use placeholder IDs — replace with mainnet feed IDs from data.chain.link/streams.
pub const ASSET_LIST: &[AssetRow] = &[
    AssetRow {
        key: "BTC",
        label: "Bitcoin",
        api_symbol: "BTCUSD",
        feed_id: "0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439",
    },
    AssetRow {
        key: "ETH",
        label: "Ethereum",
        api_symbol: "ETHUSD",
        feed_id: "0x000359843a543ee2fe414dc14c7e7920ef10f4372990b79d6361cdc0dd1ba782",
    },
    AssetRow {
        key: "SOL",
        label: "Solana",
        api_symbol: "SOLUSD",
        feed_id: "0x0000000000000000000000000000000000000000000000000000000000000000",
    },
    AssetRow {
        key: "XRP",
        label: "XRP",
        api_symbol: "XRPUSD",
        feed_id: "0x0000000000000000000000000000000000000000000000000000000000000000",
    },
];
