#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEvent {
    pub proxy_wallet: String,
    pub transaction_hash: String,
    pub side: String,
    pub asset_id: String,
    pub size: u64,
    pub observed_at_ms: u64,
}

impl ActivityEvent {
    pub fn new(
        proxy_wallet: impl Into<String>,
        transaction_hash: impl Into<String>,
        side: impl Into<String>,
        asset_id: impl Into<String>,
        size: u64,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            proxy_wallet: proxy_wallet.into(),
            transaction_hash: transaction_hash.into(),
            side: side.into(),
            asset_id: asset_id.into(),
            size,
            observed_at_ms,
        }
    }
}
