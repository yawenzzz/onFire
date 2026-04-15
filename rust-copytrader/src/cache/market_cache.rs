use std::collections::HashMap;

use crate::adapters::market_ws::MarketQuoteSnapshot;

use super::freshness::FreshnessGate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketCacheError {
    MissingAsset(String),
    Stale {
        asset_id: String,
        observed_age_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub struct MarketCache {
    freshness: FreshnessGate,
    quotes: HashMap<String, MarketQuoteSnapshot>,
}

impl MarketCache {
    pub fn new(max_quote_age_ms: u64) -> Self {
        Self {
            freshness: FreshnessGate::new(max_quote_age_ms),
            quotes: HashMap::new(),
        }
    }

    pub fn update(&mut self, quote: MarketQuoteSnapshot) -> bool {
        match self.quotes.get(&quote.asset_id) {
            Some(existing) if existing.observed_at_ms > quote.observed_at_ms => false,
            _ => {
                self.quotes.insert(quote.asset_id.clone(), quote);
                true
            }
        }
    }

    pub fn quote(
        &self,
        asset_id: impl AsRef<str>,
        now_ms: u64,
    ) -> Result<MarketQuoteSnapshot, MarketCacheError> {
        let asset_id = asset_id.as_ref();
        let quote = self
            .quotes
            .get(asset_id)
            .ok_or_else(|| MarketCacheError::MissingAsset(asset_id.to_string()))?;
        let observed_age_ms = now_ms.saturating_sub(quote.observed_at_ms);
        if !self.freshness.is_fresh(observed_age_ms) {
            return Err(MarketCacheError::Stale {
                asset_id: asset_id.to_string(),
                observed_age_ms,
            });
        }

        Ok(quote.clone())
    }
}
