//! Static asset list (mirrors `src/lib/chainlink/constants.ts`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetRow {
    pub key: &'static str,
    pub label: &'static str,
    pub api_symbol: &'static str,
}

pub const ASSET_LIST: &[AssetRow] = &[
    AssetRow {
        key: "BTC",
        label: "Bitcoin",
        api_symbol: "BTCUSD",
    },
    AssetRow {
        key: "ETH",
        label: "Ethereum",
        api_symbol: "ETHUSD",
    },
    AssetRow {
        key: "SOL",
        label: "Solana",
        api_symbol: "SOLUSD",
    },
    AssetRow {
        key: "XRP",
        label: "XRP",
        api_symbol: "XRPUSD",
    },
];
