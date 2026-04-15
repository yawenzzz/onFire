use crate::adapters::positions::PositionSnapshot;
use crate::adapters::verification::{VerificationChannelEvent, VerificationChannelKind};
use crate::domain::events::ActivityEvent;
use crate::replay::fixture::{ReplayFixture, ReplayVerificationFrame};

pub trait ActivityTransport {
    fn read_activity(&self) -> ActivityEvent;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionsFrame {
    pub previous: PositionSnapshot,
    pub current: PositionSnapshot,
    pub reconciled_at_ms: u64,
}

pub trait PositionsTransport {
    fn read_positions(&self) -> PositionsFrame;
}

pub trait MarketTransport {
    fn read_market_quote(&self) -> crate::replay::fixture::ReplayQuoteFrame;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationFrame {
    pub event: Option<VerificationChannelEvent>,
    pub observed_at_ms: u64,
}

pub trait VerificationTransport {
    fn read_verification(&self, correlation_id: &str) -> VerificationFrame;
}

#[derive(Debug, Clone)]
pub struct ReplayTransportBoundary<'a> {
    fixture: &'a ReplayFixture,
}

impl<'a> ReplayTransportBoundary<'a> {
    pub const fn new(fixture: &'a ReplayFixture) -> Self {
        Self { fixture }
    }

    pub const fn transport_name(&self) -> &'static str {
        "replay"
    }
}

impl ActivityTransport for ReplayTransportBoundary<'_> {
    fn read_activity(&self) -> ActivityEvent {
        self.fixture.activity.clone()
    }
}

impl PositionsTransport for ReplayTransportBoundary<'_> {
    fn read_positions(&self) -> PositionsFrame {
        PositionsFrame {
            previous: self.fixture.previous_position.clone(),
            current: self.fixture.current_position.clone(),
            reconciled_at_ms: self.fixture.positions_reconciled_at_ms,
        }
    }
}

impl MarketTransport for ReplayTransportBoundary<'_> {
    fn read_market_quote(&self) -> crate::replay::fixture::ReplayQuoteFrame {
        self.fixture.quote.clone()
    }
}

impl VerificationTransport for ReplayTransportBoundary<'_> {
    fn read_verification(&self, correlation_id: &str) -> VerificationFrame {
        let event = match self.fixture.verification {
            ReplayVerificationFrame::Verified { verified_at_ms } => {
                Some(VerificationChannelEvent::new(
                    correlation_id.to_string(),
                    VerificationChannelKind::OrderMatched,
                    verified_at_ms,
                ))
            }
            ReplayVerificationFrame::Mismatch { observed_at_ms } => {
                Some(VerificationChannelEvent::new(
                    correlation_id.to_string(),
                    VerificationChannelKind::OrderMismatch,
                    observed_at_ms,
                ))
            }
            ReplayVerificationFrame::Timeout { .. } => None,
        };

        VerificationFrame {
            event,
            observed_at_ms: self.fixture.verification.observed_at_ms(),
        }
    }
}
