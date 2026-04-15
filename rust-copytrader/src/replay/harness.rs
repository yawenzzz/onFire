use crate::adapters::market_ws::{MarketQuoteGate, QuoteRejection};
use crate::adapters::positions::PositionsReconciler;
use crate::domain::budget::{LatencyBudget, StageBudget};
use crate::execution::state_machine::{OrderLifecycle, SubmitFailure};
use crate::pipeline::trace_context::{Stage, TraceContext};
use crate::replay::fixture::{ReplayFixture, ReplayPreviewResult, ReplaySubmitResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayStage {
    ActivityObserved,
    PositionsReconciled,
    MarketQuoted,
    OrderSubmitted,
    VerificationObserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayRejectReason {
    Positions(String),
    Quote(QuoteRejection),
    BudgetExceeded { stage: ReplayStage, elapsed_ms: u64 },
    PreviewRejected,
    SubmitRejected,
}

impl ReplayRejectReason {
    pub fn code(&self) -> String {
        match self {
            Self::Positions(reason) => reason.clone(),
            Self::Quote(QuoteRejection::Stale) => "quote_stale".to_string(),
            Self::Quote(QuoteRejection::InvalidSpread) => "quote_invalid_spread".to_string(),
            Self::BudgetExceeded { stage, .. } => match stage {
                ReplayStage::ActivityObserved => "activity_over_budget".to_string(),
                ReplayStage::PositionsReconciled => "positions_over_budget".to_string(),
                ReplayStage::MarketQuoted => "quote_over_budget".to_string(),
                ReplayStage::OrderSubmitted => "submit_over_budget".to_string(),
                ReplayStage::VerificationObserved => "verification_over_budget".to_string(),
            },
            Self::PreviewRejected => "preview_rejected".to_string(),
            Self::SubmitRejected => "submit_rejected".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRejected {
    pub reason: ReplayRejectReason,
    pub trace: TraceContext,
    pub stage_order: Vec<ReplayStage>,
    pub lifecycle: Option<OrderLifecycle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayAccepted {
    pub lifecycle: OrderLifecycle,
    pub trace: TraceContext,
    pub stage_order: Vec<ReplayStage>,
    pub submit_elapsed_ms: u64,
    pub verification_elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayRunOutcome {
    Rejected(ReplayRejected),
    Accepted(ReplayAccepted),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayHarness {
    latency_budget: LatencyBudget,
    positions_reconciler: PositionsReconciler,
    quote_gate: MarketQuoteGate,
    quote_stage_budget: StageBudget,
    submit_stage_budget: StageBudget,
}

impl Default for ReplayHarness {
    fn default() -> Self {
        Self::new(200, 45, 10, 10, 70)
    }
}

impl ReplayHarness {
    pub fn new(
        hard_limit_ms: u64,
        max_positions_age_ms: u64,
        max_quote_age_ms: u64,
        quote_stage_target_ms: u64,
        submit_stage_target_ms: u64,
    ) -> Self {
        Self {
            latency_budget: LatencyBudget::new(hard_limit_ms),
            positions_reconciler: PositionsReconciler::new(max_positions_age_ms),
            quote_gate: MarketQuoteGate::new(max_quote_age_ms),
            quote_stage_budget: StageBudget::new("market_ws", quote_stage_target_ms),
            submit_stage_budget: StageBudget::new("submit", submit_stage_target_ms),
        }
    }

    pub fn run(&self, fixture: &ReplayFixture) -> ReplayRunOutcome {
        let mut trace = TraceContext::new(
            fixture.leader_id.clone(),
            fixture.correlation_id.clone(),
            fixture.activity.observed_at_ms,
        );
        let mut stage_order = vec![ReplayStage::ActivityObserved];
        trace.mark(Stage::ActivityObserved, fixture.activity.observed_at_ms);

        let positions_elapsed_ms = fixture
            .positions_reconciled_at_ms
            .saturating_sub(fixture.activity.observed_at_ms);
        if self
            .latency_budget
            .remaining_ms(positions_elapsed_ms)
            .is_none()
        {
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::BudgetExceeded {
                    stage: ReplayStage::PositionsReconciled,
                    elapsed_ms: positions_elapsed_ms,
                },
                trace,
                stage_order,
                lifecycle: None,
            });
        }

        let positions_outcome = self
            .positions_reconciler
            .reconcile(&fixture.previous_position, &fixture.current_position);
        let delta = match positions_outcome {
            crate::adapters::positions::PositionsOutcome::Rejected(reason) => {
                return ReplayRunOutcome::Rejected(ReplayRejected {
                    reason: ReplayRejectReason::Positions(reason),
                    trace,
                    stage_order,
                    lifecycle: None,
                });
            }
            crate::adapters::positions::PositionsOutcome::NoNetChange => {
                return ReplayRunOutcome::Rejected(ReplayRejected {
                    reason: ReplayRejectReason::Positions("positions_no_net_change".to_string()),
                    trace,
                    stage_order,
                    lifecycle: None,
                });
            }
            crate::adapters::positions::PositionsOutcome::Delta(delta) => delta,
        };

        stage_order.push(ReplayStage::PositionsReconciled);
        trace.mark(
            Stage::PositionsReconciled,
            fixture.positions_reconciled_at_ms,
        );
        if !self
            .latency_budget
            .can_schedule(positions_elapsed_ms, &self.quote_stage_budget)
        {
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::BudgetExceeded {
                    stage: ReplayStage::MarketQuoted,
                    elapsed_ms: positions_elapsed_ms,
                },
                trace,
                stage_order,
                lifecycle: None,
            });
        }

        let quote_elapsed_ms = fixture
            .quote
            .observed_at_ms
            .saturating_sub(fixture.activity.observed_at_ms);
        let quote = match self.quote_gate.validate(
            delta.asset_id,
            fixture.quote.best_bid,
            fixture.quote.best_ask,
            fixture.quote.quote_age_ms,
            fixture.quote.observed_at_ms,
        ) {
            Ok(quote) => quote,
            Err(reason) => {
                return ReplayRunOutcome::Rejected(ReplayRejected {
                    reason: ReplayRejectReason::Quote(reason),
                    trace,
                    stage_order,
                    lifecycle: None,
                });
            }
        };

        stage_order.push(ReplayStage::MarketQuoted);
        trace.mark(Stage::MarketQuoted, quote.observed_at_ms);
        if !self
            .latency_budget
            .can_schedule(quote_elapsed_ms, &self.submit_stage_budget)
        {
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::BudgetExceeded {
                    stage: ReplayStage::OrderSubmitted,
                    elapsed_ms: quote_elapsed_ms,
                },
                trace,
                stage_order,
                lifecycle: None,
            });
        }

        let mut lifecycle = OrderLifecycle::new(fixture.correlation_id.clone());
        if fixture.preview == ReplayPreviewResult::Rejected {
            lifecycle = lifecycle.mark_submit_failed(SubmitFailure::PreviewRejected);
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::PreviewRejected,
                trace,
                stage_order,
                lifecycle: Some(lifecycle),
            });
        }

        if fixture.submit == ReplaySubmitResult::Rejected {
            lifecycle = lifecycle.mark_submit_failed(SubmitFailure::SubmitRejected);
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::SubmitRejected,
                trace,
                stage_order,
                lifecycle: Some(lifecycle),
            });
        }

        let submit_elapsed_ms = fixture.submit_elapsed_ms();
        if self
            .latency_budget
            .remaining_ms(submit_elapsed_ms)
            .is_none()
        {
            return ReplayRunOutcome::Rejected(ReplayRejected {
                reason: ReplayRejectReason::BudgetExceeded {
                    stage: ReplayStage::OrderSubmitted,
                    elapsed_ms: submit_elapsed_ms,
                },
                trace,
                stage_order,
                lifecycle: None,
            });
        }

        stage_order.push(ReplayStage::OrderSubmitted);
        trace.mark(Stage::OrderSubmitted, fixture.submit_ack_at_ms);

        lifecycle = lifecycle.mark_submitted(fixture.submit_ack_at_ms);
        lifecycle = lifecycle.apply_verification(fixture.verification.into());
        stage_order.push(ReplayStage::VerificationObserved);
        trace.mark(
            Stage::VerificationObserved,
            fixture.verification.observed_at_ms(),
        );

        ReplayRunOutcome::Accepted(ReplayAccepted {
            lifecycle,
            trace,
            stage_order,
            submit_elapsed_ms,
            verification_elapsed_ms: fixture
                .verification
                .observed_at_ms()
                .saturating_sub(fixture.activity.observed_at_ms),
        })
    }
}
