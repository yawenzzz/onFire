use crate::config::{ActivityMode, LiveModeGate};
use crate::persistence::jsonl::{RotatingJsonlWriter, SessionLogKind};
use crate::persistence::snapshots::{
    LeaderStateSnapshot, RuntimeSnapshot, SessionSnapshotWriter, SnapshotBundle,
};
use crate::pipeline::orchestrator::HotPathOrchestrator;
use crate::replay::fixture::ReplayFixture;
use crate::telemetry::latency::LatencyReport;
use crate::telemetry::metrics::RuntimeMetrics;
use crate::telemetry::report::{OperatorArtifactPaths, OperatorReport};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRuntimeArtifacts {
    pub latest_snapshot_path: PathBuf,
    pub snapshot_archive_path: PathBuf,
    pub activity_log_path: PathBuf,
    pub order_log_path: PathBuf,
    pub verification_log_path: PathBuf,
    pub report_path: PathBuf,
    pub summary_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RuntimeSessionRecorder {
    root: PathBuf,
    session_id: String,
    snapshot_writer: SessionSnapshotWriter,
    activity_log_writer: RotatingJsonlWriter,
    order_log_writer: RotatingJsonlWriter,
    verification_log_writer: RotatingJsonlWriter,
}

impl RuntimeSessionRecorder {
    pub fn new(
        root: impl Into<PathBuf>,
        session_id: impl Into<String>,
        max_snapshot_archives: usize,
        max_log_records_per_file: usize,
    ) -> Self {
        let root = root.into();
        let session_id = session_id.into();
        Self {
            snapshot_writer: SessionSnapshotWriter::new(
                root.clone(),
                session_id.clone(),
                max_snapshot_archives,
            ),
            activity_log_writer: RotatingJsonlWriter::new(
                root.clone(),
                session_id.clone(),
                SessionLogKind::Activity,
                max_log_records_per_file,
            ),
            order_log_writer: RotatingJsonlWriter::new(
                root.clone(),
                session_id.clone(),
                SessionLogKind::Orders,
                max_log_records_per_file,
            ),
            verification_log_writer: RotatingJsonlWriter::new(
                root.clone(),
                session_id.clone(),
                SessionLogKind::Verification,
                max_log_records_per_file,
            ),
            root,
            session_id,
        }
    }

    pub fn persist(&mut self, session: &RuntimeSession) -> io::Result<PersistedRuntimeArtifacts> {
        let snapshot = session
            .snapshot()
            .ok_or_else(|| io::Error::other("runtime session has no snapshot to persist"))?;

        let snapshot_result = self.snapshot_writer.persist(snapshot)?;
        let activity_log_path = self
            .activity_log_writer
            .append(&activity_record(snapshot))?;
        let order_log_path = self.order_log_writer.append(&order_record(snapshot))?;
        let verification_log_path = self
            .verification_log_writer
            .append(&verification_record(snapshot))?;

        let report_path = session_root(&self.root, &self.session_id).join("operator-report.json");
        let summary_path = session_root(&self.root, &self.session_id).join("operator-summary.txt");
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let report = OperatorReport::from_runtime(
            snapshot,
            session.metrics(),
            session.latency(),
            OperatorArtifactPaths {
                latest_snapshot_path: Some(snapshot_result.latest_path.clone()),
                snapshot_archive_path: Some(snapshot_result.archive_path.clone()),
                activity_log_path: Some(activity_log_path.clone()),
                order_log_path: Some(order_log_path.clone()),
                verification_log_path: Some(verification_log_path.clone()),
                report_path: Some(report_path.clone()),
                summary_path: Some(summary_path.clone()),
            },
        );

        fs::write(&report_path, report.render_json())?;
        fs::write(&summary_path, report.render_summary())?;

        Ok(PersistedRuntimeArtifacts {
            latest_snapshot_path: snapshot_result.latest_path,
            snapshot_archive_path: snapshot_result.archive_path,
            activity_log_path,
            order_log_path,
            verification_log_path,
            report_path,
            summary_path,
        })
    }
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

fn activity_record(snapshot: &SnapshotBundle) -> String {
    format!(
        concat!(
            "{{",
            "\"leader_id\":\"{}\",",
            "\"last_activity_at_ms\":{},",
            "\"last_transaction_hash\":\"{}\"",
            "}}"
        ),
        escape_json(&snapshot.leader.leader_id),
        snapshot.leader.last_activity_at_ms,
        escape_json(&snapshot.leader.last_transaction_hash),
    )
}

fn order_record(snapshot: &SnapshotBundle) -> String {
    format!(
        concat!(
            "{{",
            "\"last_submit_status\":\"{}\",",
            "\"last_correlation_id\":{},",
            "\"last_reject_reason\":{},",
            "\"last_stage\":{},",
            "\"last_total_elapsed_ms\":{}",
            "}}"
        ),
        escape_json(&snapshot.runtime.last_submit_status),
        opt_json(snapshot.runtime.last_correlation_id.as_deref()),
        opt_json(snapshot.runtime.last_reject_reason.as_deref()),
        opt_json(snapshot.runtime.last_stage.as_deref()),
        snapshot.runtime.last_total_elapsed_ms,
    )
}

fn verification_record(snapshot: &SnapshotBundle) -> String {
    format!(
        concat!(
            "{{",
            "\"verification_pending\":{},",
            "\"last_submit_status\":\"{}\",",
            "\"last_correlation_id\":{}",
            "}}"
        ),
        snapshot.runtime.verification_pending,
        escape_json(&snapshot.runtime.last_submit_status),
        opt_json(snapshot.runtime.last_correlation_id.as_deref()),
    )
}

fn opt_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn session_root(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions").join(session_id)
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
