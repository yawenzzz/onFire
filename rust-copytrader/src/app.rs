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
        if let BootstrapDecision::Blocked(reason) = decision {
            return SessionOutcome::Blocked(reason);
        }

        let outcome = self.orchestrator.run(fixture);
        self.latency.record_trace(outcome.trace());

        let mut verification_pending = 0_u64;
        let last_submit_status = if let Some(reason) = outcome.reject_reason() {
            self.metrics.record_reject(reason);
            format!("rejected:{reason}")
        } else if let Some(lifecycle) = outcome.lifecycle() {
            let label = lifecycle.status_label().to_string();
            match label.as_str() {
                "submit_failed" => self.metrics.record_reject("submit_failed"),
                "submitted_unverified" => {
                    self.metrics.record_submit();
                    verification_pending = 1;
                }
                "verification_timeout" => {
                    self.metrics.record_submit();
                    self.metrics.record_verification_timeout();
                }
                "verified" | "verification_mismatch" => self.metrics.record_submit(),
                _ => {}
            }
            label
        } else {
            "unknown".to_string()
        };

        let runtime_snapshot = RuntimeSnapshot {
            mode: mode_label(&decision).to_string(),
            live_mode_unlocked: matches!(decision, BootstrapDecision::LiveListen),
            blocked_reason: None,
            verification_pending,
            last_submit_status,
        };
        let leader_snapshot = LeaderStateSnapshot {
            leader_id: fixture.activity.proxy_wallet.clone(),
            last_activity_at_ms: fixture.activity.observed_at_ms,
            last_transaction_hash: fixture.activity.transaction_hash.clone(),
            last_position_size: fixture.current_position.current_size,
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
