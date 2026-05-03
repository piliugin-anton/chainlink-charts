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
        feed_id: "0x0003c74bfa2f66d6c2f6e1f3b37b4b44a1d2c3e5f6a7b8c9d0e1f2a3b4c5d6e7",
    },
    AssetRow {
        key: "XRP",
        label: "XRP",
        api_symbol: "XRPUSD",
        feed_id: "0x0003d85e2b1c3a4f5e6d7c8b9a0e1f2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
    },
];
