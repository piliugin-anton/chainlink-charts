//! Static asset list (mirrors `src/lib/chainlink/constants.ts`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetRow {
    pub key: &'static str,
    pub label: &'static str,
    pub api_symbol: &'static str,
    /// Mainnet Data Streams feed ID (hex); WebSocket host is `wss://ws.dataengine.chain.link` per Chainlink docs.
    pub feed_id_mainnet: &'static str,
    /// `api.testnet-dataengine.chain.link` (e.g. Arbitrum Sepolia); SOL/XRP may need replacing from your dashboard.
    pub feed_id_testnet: &'static str,
}

/// Chainlink Data Streams feed IDs for WebSocket streaming (`stream_loop` picks mainnet vs testnet from `CHAINLINK_BASE_URL`).
pub const ASSET_LIST: &[AssetRow] = &[
    AssetRow {
        key: "BTC",
        label: "Bitcoin",
        api_symbol: "BTCUSD",
        feed_id_mainnet: "0x00039d9e45394f473ab1f050a1b963e6b05351e52d71e507509ada0c95ed75b8",
        feed_id_testnet: "0x00037da06d56d083fe599397a4769a042d63aa73dc4ef57709d31e9971a5b439",
    },
    AssetRow {
        key: "ETH",
        label: "Ethereum",
        api_symbol: "ETHUSD",
        feed_id_mainnet: "0x000362205e10b3a147d02792eccee483dca6c7b44ecce7012cb8c6e0b68b3ae9",
        feed_id_testnet: "0x000359843a543ee2fe414dc14c7e7920ef10f4372990b79d6361cdc0dd1ba782",
    },
    AssetRow {
        key: "SOL",
        label: "Solana",
        api_symbol: "SOLUSD",
        feed_id_mainnet: "0x0003b778d3f6b2ac4991302b89cb313f99a42467d6c9c5f96f57c29c0d2bc24f",
        feed_id_testnet: "0x0003c74bfa2f66d6c2f6e1f3b37b4b44a1d2c3e5f6a7b8c9d0e1f2a3b4c5d6e7",
    },
    AssetRow {
        key: "XRP",
        label: "XRP",
        api_symbol: "XRPUSD",
        feed_id_mainnet: "0x0003c16c6aed42294f5cb4741f6e59ba2d728f0eae2eb9e6d3f555808c59fc45",
        feed_id_testnet: "0x0003c16c6aed42294f5cb4741f6e59ba2d728f0eae2eb9e6d3f555808c59fc45",
    },
];
