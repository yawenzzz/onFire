use crate::cache::freshness::FreshnessGate;

#[derive(Debug, Clone, PartialEq)]
pub struct MarketQuoteSnapshot {
    pub asset_id: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteRejection {
    Stale,
    InvalidSpread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketQuoteGate {
    freshness: FreshnessGate,
}

impl MarketQuoteGate {
    pub const fn new(max_age_ms: u64) -> Self {
        Self {
            freshness: FreshnessGate::new(max_age_ms),
        }
    }

    pub fn validate(
        &self,
        asset_id: impl Into<String>,
        best_bid: f64,
        best_ask: f64,
        quote_age_ms: u64,
        observed_at_ms: u64,
    ) -> Result<MarketQuoteSnapshot, QuoteRejection> {
        if !self.freshness.is_fresh(quote_age_ms) {
            return Err(QuoteRejection::Stale);
        }
        if best_bid <= 0.0 || best_ask <= 0.0 || best_bid > best_ask {
            return Err(QuoteRejection::InvalidSpread);
        }

        Ok(MarketQuoteSnapshot {
            asset_id: asset_id.into(),
            best_bid,
            best_ask,
            observed_at_ms,
        })
    }
}
