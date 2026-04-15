use crate::adapters::positions::PositionSnapshot;
use crate::domain::events::ActivityEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct PositionFixtures {
    pub previous: PositionSnapshot,
    pub current: PositionSnapshot,
}

impl PositionFixtures {
    pub fn new(
        previous: (&str, &str, i64, u64, u64),
        current: (&str, &str, i64, u64, u64),
    ) -> Self {
        Self {
            previous: PositionSnapshot::new(
                previous.0, previous.1, previous.2, previous.3, previous.4,
            ),
            current: PositionSnapshot::new(current.0, current.1, current.2, current.3, current.4),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketFixture {
    pub asset_id: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub quote_age_ms: u64,
    pub observed_at_ms: u64,
    pub market_open: bool,
}

impl MarketFixture {
    pub fn new(
        asset_id: impl Into<String>,
        best_bid: f64,
        best_ask: f64,
        quote_age_ms: u64,
        observed_at_ms: u64,
        market_open: bool,
    ) -> Self {
        Self {
            asset_id: asset_id.into(),
            best_bid,
            best_ask,
            quote_age_ms,
            observed_at_ms,
            market_open,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitFixture {
    Accepted {
        order_id: String,
        submitted_at_ms: u64,
    },
    Rejected,
}

impl SubmitFixture {
    pub fn accepted(order_id: impl Into<String>, submitted_at_ms: u64) -> Self {
        Self::Accepted {
            order_id: order_id.into(),
            submitted_at_ms,
        }
    }

    pub const fn rejected() -> Self {
        Self::Rejected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationFixture {
    Verified { verified_at_ms: u64 },
    Mismatch { observed_at_ms: u64 },
    Timeout { observed_at_ms: u64 },
}

impl VerificationFixture {
    pub const fn verified(verified_at_ms: u64) -> Self {
        Self::Verified { verified_at_ms }
    }

    pub const fn mismatch(observed_at_ms: u64) -> Self {
        Self::Mismatch { observed_at_ms }
    }

    pub const fn timeout(observed_at_ms: u64) -> Self {
        Self::Timeout { observed_at_ms }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayFixture {
    pub activity: ActivityEvent,
    pub positions: PositionFixtures,
    pub market: MarketFixture,
    pub preview_ok: bool,
    pub submit: SubmitFixture,
    pub verification: Option<VerificationFixture>,
}
