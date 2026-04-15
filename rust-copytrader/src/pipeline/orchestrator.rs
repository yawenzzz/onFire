use crate::adapters::market_ws::{MarketQuoteGate, QuoteRejection};
use crate::adapters::order_api::{OrderIntent, OrderSide};
use crate::adapters::positions::{PositionsOutcome, PositionsReconciler};
use crate::adapters::transport::{
    ActivityTransport, MarketTransport, PositionsTransport, ReplayTransportBoundary,
    VerificationTransport,
};
use crate::adapters::verification::VerificationChannelKind;
use crate::domain::budget::{LatencyBudget, StageBudget};
use crate::execution::pre_trade_gate::{PreTradeGate, PreTradeInput};
use crate::execution::state_machine::{OrderLifecycle, SubmitFailure};
use crate::pipeline::trace_context::{Stage, TraceContext};
use crate::replay::fixture::{ReplayFixture, ReplayPreviewResult, ReplaySubmitResult};

#[derive(Debug, Clone)]
pub struct HotPathOrchestrator {
    total_budget: LatencyBudget,
    submit_stage: StageBudget,
    positions_reconciler: PositionsReconciler,
    quote_gate: MarketQuoteGate,
    pre_trade_gate: PreTradeGate,
}

impl Default for HotPathOrchestrator {
    fn default() -> Self {
        Self {
            total_budget: LatencyBudget::new(200),
            submit_stage: StageBudget::new("preview_submit", 70),
            positions_reconciler: PositionsReconciler::new(45),
            quote_gate: MarketQuoteGate::new(10),
            pre_trade_gate: PreTradeGate::new(70),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineOutcome {
    trace: TraceContext,
    reject_reason: Option<String>,
    lifecycle: Option<OrderLifecycle>,
    order_intent: Option<OrderIntent>,
}

impl PipelineOutcome {
    fn rejected(trace: TraceContext, reason: impl Into<String>) -> Self {
        Self {
            trace,
            reject_reason: Some(reason.into()),
            lifecycle: None,
            order_intent: None,
        }
    }

    fn with_lifecycle(
        trace: TraceContext,
        lifecycle: OrderLifecycle,
        order_intent: Option<OrderIntent>,
    ) -> Self {
        Self {
            trace,
            reject_reason: None,
            lifecycle: Some(lifecycle),
            order_intent,
        }
    }

    pub fn trace(&self) -> &TraceContext {
        &self.trace
    }

    pub fn reject_reason(&self) -> Option<&str> {
        self.reject_reason.as_deref()
    }

    pub fn lifecycle(&self) -> Option<&OrderLifecycle> {
        self.lifecycle.as_ref()
    }

    pub fn lifecycle_status_label(&self) -> Option<&str> {
        self.lifecycle.as_ref().map(OrderLifecycle::status_label)
    }

    pub fn order_side(&self) -> Option<OrderSide> {
        self.order_intent.as_ref().map(|intent| intent.side)
    }
}

impl HotPathOrchestrator {
    pub fn run(&self, fixture: &ReplayFixture) -> PipelineOutcome {
        let transport = ReplayTransportBoundary::new(fixture);
        self.run_transport(&transport, fixture)
    }

    pub fn run_transport<T>(&self, transport: &T, fixture: &ReplayFixture) -> PipelineOutcome
    where
        T: ActivityTransport + PositionsTransport + MarketTransport + VerificationTransport,
    {
        let activity = transport.read_activity();
        let positions = transport.read_positions();
        let quote_frame = transport.read_market_quote();
        let verification_frame = transport.read_verification(&fixture.correlation_id);

        let mut trace = TraceContext::new(
            activity.proxy_wallet.clone(),
            activity.transaction_hash.clone(),
            activity.observed_at_ms,
        );
        trace.mark(Stage::ActivityObserved, activity.observed_at_ms);

        let delta = match self
            .positions_reconciler
            .reconcile(&positions.previous, &positions.current)
        {
            PositionsOutcome::Rejected(reason) => return PipelineOutcome::rejected(trace, reason),
            PositionsOutcome::NoNetChange => {
                return PipelineOutcome::rejected(trace, "no_net_position_change");
            }
            PositionsOutcome::Delta(delta) => delta,
        };
        trace.mark(Stage::PositionsReconciled, positions.reconciled_at_ms);

        let quote = match self.quote_gate.validate(
            quote_frame.asset_id.clone(),
            quote_frame.best_bid,
            quote_frame.best_ask,
            quote_frame.quote_age_ms,
            quote_frame.observed_at_ms,
        ) {
            Ok(quote) => quote,
            Err(error) => {
                let reason = match error {
                    QuoteRejection::Stale => "quote_stale",
                    QuoteRejection::InvalidSpread => "quote_invalid_spread",
                };
                return PipelineOutcome::rejected(trace, reason);
            }
        };
        trace.mark(Stage::MarketQuoted, quote.observed_at_ms);

        let elapsed_ms = quote.observed_at_ms.saturating_sub(activity.observed_at_ms);
        if !self
            .total_budget
            .can_schedule(elapsed_ms, &self.submit_stage)
        {
            return PipelineOutcome::rejected(trace, "latency_budget_exhausted");
        }

        let order_side = if delta.delta_size > 0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let limit_price = match order_side {
            OrderSide::Buy => quote.best_ask,
            OrderSide::Sell => quote.best_bid,
        };
        let remaining_budget_ms = self
            .total_budget
            .remaining_ms(elapsed_ms)
            .unwrap_or_default();
        let preview_ok = fixture.preview == ReplayPreviewResult::Accepted;
        let order_intent = match self.pre_trade_gate.evaluate(PreTradeInput {
            asset_id: delta.asset_id.clone(),
            side: order_side,
            size: delta.delta_size.unsigned_abs(),
            limit_price,
            market_open: quote_frame.market_open,
            preview_ok,
            remaining_budget_ms,
        }) {
            Ok(intent) => intent,
            Err(reason) => return PipelineOutcome::rejected(trace, reason),
        };
        trace.mark(Stage::PreTradeValidated, quote.observed_at_ms);

        let base_lifecycle = OrderLifecycle::new(trace.correlation_id().to_string());
        if fixture.preview == ReplayPreviewResult::Rejected {
            return PipelineOutcome::with_lifecycle(
                trace,
                base_lifecycle.mark_submit_failed(SubmitFailure::PreviewRejected),
                Some(order_intent),
            );
        }

        if fixture.submit == ReplaySubmitResult::Rejected {
            return PipelineOutcome::with_lifecycle(
                trace,
                base_lifecycle.mark_submit_failed(SubmitFailure::SubmitRejected),
                Some(order_intent),
            );
        }

        if fixture
            .submit_ack_at_ms
            .saturating_sub(activity.observed_at_ms)
            > self.total_budget.hard_limit_ms()
        {
            return PipelineOutcome::rejected(trace, "latency_budget_exhausted");
        }

        trace.mark(Stage::OrderSubmitted, fixture.submit_ack_at_ms);
        let lifecycle = base_lifecycle.mark_submitted(fixture.submit_ack_at_ms);
        trace.mark(
            Stage::VerificationObserved,
            verification_frame.observed_at_ms,
        );
        let lifecycle = lifecycle.apply_verification(match verification_frame.event {
            Some(event) => match event.kind {
                VerificationChannelKind::OrderMatched => {
                    crate::execution::state_machine::VerificationOutcome::Verified {
                        verified_at_ms: event.observed_at_ms,
                    }
                }
                VerificationChannelKind::OrderMismatch => {
                    crate::execution::state_machine::VerificationOutcome::Mismatch {
                        observed_at_ms: event.observed_at_ms,
                    }
                }
            },
            None => crate::execution::state_machine::VerificationOutcome::Timeout {
                observed_at_ms: verification_frame.observed_at_ms,
            },
        });

        PipelineOutcome::with_lifecycle(trace, lifecycle, Some(order_intent))
    }
}
