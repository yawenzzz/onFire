use crate::adapters::positions::PositionSnapshot;
use crate::domain::events::ActivityEvent;
use crate::execution::state_machine::VerificationOutcome;

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayQuoteFrame {
    pub asset_id: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub quote_age_ms: u64,
    pub observed_at_ms: u64,
    pub market_open: bool,
}

impl ReplayQuoteFrame {
    pub fn new(
        asset_id: impl Into<String>,
        best_bid: f64,
        best_ask: f64,
        quote_age_ms: u64,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            asset_id: asset_id.into(),
            best_bid,
            best_ask,
            quote_age_ms,
            observed_at_ms,
            market_open: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayPreviewResult {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaySubmitResult {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayVerificationFrame {
    Verified { verified_at_ms: u64 },
    Mismatch { observed_at_ms: u64 },
    Timeout { observed_at_ms: u64 },
}

impl ReplayVerificationFrame {
    pub const fn observed_at_ms(&self) -> u64 {
        match self {
            Self::Verified { verified_at_ms } => *verified_at_ms,
            Self::Mismatch { observed_at_ms } | Self::Timeout { observed_at_ms } => *observed_at_ms,
        }
    }
}

impl From<ReplayVerificationFrame> for VerificationOutcome {
    fn from(value: ReplayVerificationFrame) -> Self {
        match value {
            ReplayVerificationFrame::Verified { verified_at_ms } => {
                VerificationOutcome::Verified { verified_at_ms }
            }
            ReplayVerificationFrame::Mismatch { observed_at_ms } => {
                VerificationOutcome::Mismatch { observed_at_ms }
            }
            ReplayVerificationFrame::Timeout { observed_at_ms } => {
                VerificationOutcome::Timeout { observed_at_ms }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayFixture {
    pub scenario_name: String,
    pub leader_id: String,
    pub correlation_id: String,
    pub activity: ActivityEvent,
    pub previous_position: PositionSnapshot,
    pub current_position: PositionSnapshot,
    pub positions_reconciled_at_ms: u64,
    pub quote: ReplayQuoteFrame,
    pub preview: ReplayPreviewResult,
    pub submit: ReplaySubmitResult,
    pub submit_ack_at_ms: u64,
    pub verification: ReplayVerificationFrame,
}

impl ReplayFixture {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scenario_name: impl Into<String>,
        leader_id: impl Into<String>,
        correlation_id: impl Into<String>,
        activity: ActivityEvent,
        previous_position: PositionSnapshot,
        current_position: PositionSnapshot,
        positions_reconciled_at_ms: u64,
        quote: ReplayQuoteFrame,
        preview: ReplayPreviewResult,
        submit: ReplaySubmitResult,
        submit_ack_at_ms: u64,
        verification: ReplayVerificationFrame,
    ) -> Self {
        Self {
            scenario_name: scenario_name.into(),
            leader_id: leader_id.into(),
            correlation_id: correlation_id.into(),
            activity,
            previous_position,
            current_position,
            positions_reconciled_at_ms,
            quote,
            preview,
            submit,
            submit_ack_at_ms,
            verification,
        }
    }

    pub fn success_buy_follow() -> Self {
        Self::new(
            "success_buy_follow",
            "leader-1",
            "corr-success",
            ActivityEvent::new("leader-1", "0xtx-success", "BUY", "asset-9", 4, 1_000),
            PositionSnapshot::new("leader-1", "asset-9", 10, 995, 5),
            PositionSnapshot::new("leader-1", "asset-9", 14, 1_020, 5),
            1_020,
            ReplayQuoteFrame::new("asset-9", 0.48, 0.52, 6, 1_028),
            ReplayPreviewResult::Accepted,
            ReplaySubmitResult::Accepted,
            1_060,
            ReplayVerificationFrame::Verified {
                verified_at_ms: 1_082,
            },
        )
    }

    pub fn submit_elapsed_ms(&self) -> u64 {
        self.submit_ack_at_ms
            .saturating_sub(self.activity.observed_at_ms)
    }
}
