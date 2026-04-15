use crate::config::{ActivityMode, LiveModeGate};
use crate::persistence::snapshots::{LeaderStateSnapshot, RuntimeSnapshot, SnapshotBundle};
use crate::pipeline::orchestrator::HotPathOrchestrator;
use crate::replay::fixture::ReplayFixture;
use crate::telemetry::latency::LatencyReport;
use crate::telemetry::metrics::RuntimeMetrics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapDecision {
    LiveListen,
    ShadowPoll,
    Replay,
    Blocked(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeBootstrap {
    requested_mode: ActivityMode,
    gate: LiveModeGate,
}

impl RuntimeBootstrap {
    pub const fn new(requested_mode: ActivityMode, gate: LiveModeGate) -> Self {
        Self {
            requested_mode,
            gate,
        }
    }

    pub fn decide(&self) -> BootstrapDecision {
        match self.requested_mode {
            ActivityMode::LiveListen => self
                .gate
                .blocked_reason()
                .map(BootstrapDecision::Blocked)
                .unwrap_or(BootstrapDecision::LiveListen),
            ActivityMode::ShadowPoll => BootstrapDecision::ShadowPoll,
            ActivityMode::Replay => BootstrapDecision::Replay,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionOutcome {
    Blocked(String),
    Processed,
}

#[derive(Debug, Clone)]
pub struct RuntimeSession {
    bootstrap: RuntimeBootstrap,
    orchestrator: HotPathOrchestrator,
    metrics: RuntimeMetrics,
    latency: LatencyReport,
    latest_snapshot: Option<SnapshotBundle>,
}

impl RuntimeSession {
    pub fn new(requested_mode: ActivityMode, gate: LiveModeGate) -> Self {
        Self {
            bootstrap: RuntimeBootstrap::new(requested_mode, gate),
            orchestrator: HotPathOrchestrator::default(),
            metrics: RuntimeMetrics::default(),
            latency: LatencyReport::default(),
            latest_snapshot: None,
        }
    }

    pub fn process_replay(&mut self, fixture: &ReplayFixture) -> SessionOutcome {
        let decision = self.bootstrap.decide();
        let leader_snapshot = LeaderStateSnapshot {
            leader_id: fixture.activity.proxy_wallet.clone(),
            last_activity_at_ms: fixture.activity.observed_at_ms,
            last_transaction_hash: fixture.activity.transaction_hash.clone(),
            last_position_size: fixture.current_position.current_size,
        };

        if let BootstrapDecision::Blocked(ref reason) = decision {
            self.latest_snapshot = Some(SnapshotBundle {
                leader: leader_snapshot,
                runtime: RuntimeSnapshot {
                    mode: mode_label(&decision).to_string(),
                    live_mode_unlocked: false,
                    blocked_reason: Some(reason.clone()),
                    verification_pending: 0,
                    last_submit_status: format!("blocked:{reason}"),
                    last_correlation_id: None,
                    last_reject_reason: None,
                    last_stage: None,
                    last_total_elapsed_ms: 0,
                },
            });
            return SessionOutcome::Blocked(reason.to_string());
        }

        let outcome = self.orchestrator.run(fixture);
        self.latency.record_trace(outcome.trace());

        let mut verification_pending = 0_u64;
        let rejected_reason = outcome.reject_reason().map(str::to_string);
        let (last_submit_status, last_reject_reason) = if let Some(reason) = rejected_reason.clone()
        {
            self.metrics.record_reject(&reason);
            (format!("rejected:{reason}"), Some(reason))
        } else if let Some(lifecycle) = outcome.lifecycle() {
            let label = lifecycle.status_label().to_string();
            match label.as_str() {
                "submit_failed" => {
                    if fixture.preview == crate::replay::fixture::ReplayPreviewResult::Rejected {
                        self.metrics.record_reject("preview_rejected");
                        ("preview_rejected".to_string(), None)
                    } else if fixture.submit == crate::replay::fixture::ReplaySubmitResult::Rejected
                    {
                        self.metrics.record_reject("submit_rejected");
                        ("submit_rejected".to_string(), None)
                    } else {
                        self.metrics.record_reject("submit_failed");
                        (label, None)
                    }
                }
                "submitted_unverified" => {
                    self.metrics.record_submit();
                    verification_pending = 1;
                    (label, None)
                }
                "verification_timeout" => {
                    self.metrics.record_submit();
                    self.metrics.record_verification_timeout();
                    (label, None)
                }
                "verified" => {
                    self.metrics.record_submit();
                    self.metrics.record_verified();
                    (label, None)
                }
                "verification_mismatch" => {
                    self.metrics.record_submit();
                    self.metrics.record_verification_mismatch();
                    (label, None)
                }
                _ => (label, None),
            }
        } else {
            ("unknown".to_string(), None)
        };

        let runtime_snapshot = RuntimeSnapshot {
            mode: mode_label(&decision).to_string(),
            live_mode_unlocked: matches!(decision, BootstrapDecision::LiveListen),
            blocked_reason: None,
            verification_pending,
            last_submit_status,
            last_correlation_id: Some(if rejected_reason.is_some() {
                fixture.activity.transaction_hash.clone()
            } else {
                fixture.correlation_id.clone()
            }),
            last_reject_reason,
            last_stage: outcome.trace().last_stage().map(stage_label),
            last_total_elapsed_ms: outcome.trace().total_elapsed_ms(),
        };
        self.latest_snapshot = Some(SnapshotBundle {
            leader: leader_snapshot,
            runtime: runtime_snapshot,
        });

        SessionOutcome::Processed
    }

    pub const fn metrics(&self) -> &RuntimeMetrics {
        &self.metrics
    }

    pub const fn latency(&self) -> &LatencyReport {
        &self.latency
    }

    pub const fn snapshot(&self) -> Option<&SnapshotBundle> {
        self.latest_snapshot.as_ref()
    }
}

fn mode_label(decision: &BootstrapDecision) -> &'static str {
    match decision {
        BootstrapDecision::LiveListen => "live_listen",
        BootstrapDecision::ShadowPoll => "shadow_poll",
        BootstrapDecision::Replay => "replay",
        BootstrapDecision::Blocked(_) => "blocked",
    }
}

fn stage_label(stage: crate::pipeline::trace_context::Stage) -> String {
    match stage {
        crate::pipeline::trace_context::Stage::ActivityObserved => "activity_observed",
        crate::pipeline::trace_context::Stage::PositionsReconciled => "positions_reconciled",
        crate::pipeline::trace_context::Stage::MarketQuoted => "market_quoted",
        crate::pipeline::trace_context::Stage::PreTradeValidated => "pre_trade_validated",
        crate::pipeline::trace_context::Stage::OrderSubmitted => "order_submitted",
        crate::pipeline::trace_context::Stage::VerificationObserved => "verification_observed",
    }
    .to_string()
}
