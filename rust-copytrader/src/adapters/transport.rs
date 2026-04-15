use crate::adapters::positions::PositionSnapshot;
use crate::adapters::verification::{VerificationChannelEvent, VerificationChannelKind};
use crate::config::{ActivityMode, LiveModeGate, TransportBoundaryConfig};
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

#[derive(Debug, Clone)]
pub struct ShadowPollTransportBoundary<'a> {
    fixture: &'a ReplayFixture,
}

impl<'a> ShadowPollTransportBoundary<'a> {
    pub const fn new(fixture: &'a ReplayFixture) -> Self {
        Self { fixture }
    }

    pub const fn transport_name(&self) -> &'static str {
        "shadow_poll"
    }
}

#[derive(Debug, Clone)]
pub struct LiveListenTransportBoundary<'a> {
    fixture: &'a ReplayFixture,
}

impl<'a> LiveListenTransportBoundary<'a> {
    pub const fn new(fixture: &'a ReplayFixture) -> Self {
        Self { fixture }
    }

    pub const fn transport_name(&self) -> &'static str {
        "live_listen"
    }
}

#[derive(Debug, Clone)]
pub enum SelectedTransportBoundary<'a> {
    Replay(ReplayTransportBoundary<'a>),
    ShadowPoll(ShadowPollTransportBoundary<'a>),
    LiveListen(LiveListenTransportBoundary<'a>),
}

pub fn select_transport_boundary<'a>(
    config: TransportBoundaryConfig,
    gate: LiveModeGate,
    fixture: &'a ReplayFixture,
) -> Result<SelectedTransportBoundary<'a>, String> {
    match config.requested_mode()? {
        ActivityMode::Replay => Ok(SelectedTransportBoundary::Replay(
            ReplayTransportBoundary::new(fixture),
        )),
        ActivityMode::ShadowPoll => Ok(SelectedTransportBoundary::ShadowPoll(
            ShadowPollTransportBoundary::new(fixture),
        )),
        ActivityMode::LiveListen => gate.blocked_reason().map_or_else(
            || {
                Ok(SelectedTransportBoundary::LiveListen(
                    LiveListenTransportBoundary::new(fixture),
                ))
            },
            Err,
        ),
    }
}

impl SelectedTransportBoundary<'_> {
    pub const fn transport_name(&self) -> &'static str {
        match self {
            Self::Replay(boundary) => boundary.transport_name(),
            Self::ShadowPoll(boundary) => boundary.transport_name(),
            Self::LiveListen(boundary) => boundary.transport_name(),
        }
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
        verification_frame(self.fixture, correlation_id)
    }
}

impl ActivityTransport for ShadowPollTransportBoundary<'_> {
    fn read_activity(&self) -> ActivityEvent {
        self.fixture.activity.clone()
    }
}

impl PositionsTransport for ShadowPollTransportBoundary<'_> {
    fn read_positions(&self) -> PositionsFrame {
        PositionsFrame {
            previous: self.fixture.previous_position.clone(),
            current: self.fixture.current_position.clone(),
            reconciled_at_ms: self.fixture.positions_reconciled_at_ms,
        }
    }
}

impl MarketTransport for ShadowPollTransportBoundary<'_> {
    fn read_market_quote(&self) -> crate::replay::fixture::ReplayQuoteFrame {
        self.fixture.quote.clone()
    }
}

impl VerificationTransport for ShadowPollTransportBoundary<'_> {
    fn read_verification(&self, correlation_id: &str) -> VerificationFrame {
        verification_frame(self.fixture, correlation_id)
    }
}

impl ActivityTransport for LiveListenTransportBoundary<'_> {
    fn read_activity(&self) -> ActivityEvent {
        self.fixture.activity.clone()
    }
}

impl PositionsTransport for LiveListenTransportBoundary<'_> {
    fn read_positions(&self) -> PositionsFrame {
        PositionsFrame {
            previous: self.fixture.previous_position.clone(),
            current: self.fixture.current_position.clone(),
            reconciled_at_ms: self.fixture.positions_reconciled_at_ms,
        }
    }
}

impl MarketTransport for LiveListenTransportBoundary<'_> {
    fn read_market_quote(&self) -> crate::replay::fixture::ReplayQuoteFrame {
        self.fixture.quote.clone()
    }
}

impl VerificationTransport for LiveListenTransportBoundary<'_> {
    fn read_verification(&self, correlation_id: &str) -> VerificationFrame {
        verification_frame(self.fixture, correlation_id)
    }
}

impl ActivityTransport for SelectedTransportBoundary<'_> {
    fn read_activity(&self) -> ActivityEvent {
        match self {
            Self::Replay(boundary) => boundary.read_activity(),
            Self::ShadowPoll(boundary) => boundary.read_activity(),
            Self::LiveListen(boundary) => boundary.read_activity(),
        }
    }
}

impl PositionsTransport for SelectedTransportBoundary<'_> {
    fn read_positions(&self) -> PositionsFrame {
        match self {
            Self::Replay(boundary) => boundary.read_positions(),
            Self::ShadowPoll(boundary) => boundary.read_positions(),
            Self::LiveListen(boundary) => boundary.read_positions(),
        }
    }
}

impl MarketTransport for SelectedTransportBoundary<'_> {
    fn read_market_quote(&self) -> crate::replay::fixture::ReplayQuoteFrame {
        match self {
            Self::Replay(boundary) => boundary.read_market_quote(),
            Self::ShadowPoll(boundary) => boundary.read_market_quote(),
            Self::LiveListen(boundary) => boundary.read_market_quote(),
        }
    }
}

impl VerificationTransport for SelectedTransportBoundary<'_> {
    fn read_verification(&self, correlation_id: &str) -> VerificationFrame {
        match self {
            Self::Replay(boundary) => boundary.read_verification(correlation_id),
            Self::ShadowPoll(boundary) => boundary.read_verification(correlation_id),
            Self::LiveListen(boundary) => boundary.read_verification(correlation_id),
        }
    }
}

fn verification_frame(fixture: &ReplayFixture, correlation_id: &str) -> VerificationFrame {
    let event = match fixture.verification {
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
        observed_at_ms: fixture.verification.observed_at_ms(),
    }
}
