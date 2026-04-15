use crate::persistence::snapshots::{SnapshotBundle, escape_json};
use crate::pipeline::trace_context::Stage;
use crate::telemetry::latency::LatencyReport;
use crate::telemetry::metrics::RuntimeMetrics;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorArtifactPaths {
    pub latest_snapshot_path: Option<PathBuf>,
    pub snapshot_archive_path: Option<PathBuf>,
    pub activity_log_path: Option<PathBuf>,
    pub order_log_path: Option<PathBuf>,
    pub verification_log_path: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
    pub summary_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorReport {
    snapshot: SnapshotBundle,
    metrics: RuntimeMetrics,
    latency: LatencyReport,
    artifacts: OperatorArtifactPaths,
}

impl OperatorReport {
    pub fn from_runtime(
        snapshot: &SnapshotBundle,
        metrics: &RuntimeMetrics,
        latency: &LatencyReport,
        artifacts: OperatorArtifactPaths,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            metrics: metrics.clone(),
            latency: latency.clone(),
            artifacts,
        }
    }

    pub fn render_summary(&self) -> String {
        let blocked_reason = self
            .snapshot
            .runtime
            .blocked_reason
            .as_deref()
            .unwrap_or("none");
        format!(
            concat!(
                "mode={} ",
                "live_mode_unlocked={} ",
                "blocked_reason={} ",
                "last_submit_status={} ",
                "submitted={} ",
                "verified_total={} ",
                "rejected_total={} ",
                "latency_samples={} ",
                "max_total_elapsed_ms={}"
            ),
            self.snapshot.runtime.mode,
            self.snapshot.runtime.live_mode_unlocked,
            blocked_reason,
            self.snapshot.runtime.last_submit_status,
            self.metrics.submitted(),
            self.metrics.verified_total(),
            self.metrics.rejected_total(),
            self.latency.samples(),
            self.latency.total_elapsed_max_ms(),
        )
    }

    pub fn render_json(&self) -> String {
        let reject_counts = render_string_u64_map(
            self.metrics
                .reject_counts()
                .iter()
                .map(|(key, value)| (key.as_str(), *value)),
        );
        let stage_deltas = render_string_u64_map(
            self.latency
                .latest_stage_deltas()
                .iter()
                .map(|(stage, value)| (stage_label(*stage), *value)),
        );
        let artifact_paths = render_optional_path_map([
            ("latest_snapshot_path", self.artifacts.latest_snapshot_path.as_ref()),
            ("snapshot_archive_path", self.artifacts.snapshot_archive_path.as_ref()),
            ("activity_log_path", self.artifacts.activity_log_path.as_ref()),
            ("order_log_path", self.artifacts.order_log_path.as_ref()),
            (
                "verification_log_path",
                self.artifacts.verification_log_path.as_ref(),
            ),
            ("report_path", self.artifacts.report_path.as_ref()),
            ("summary_path", self.artifacts.summary_path.as_ref()),
        ]);
        format!(
            concat!(
                "{{",
                "\"runtime\":{},",
                "\"metrics\":{{",
                "\"submitted\":{},",
                "\"verified_total\":{},",
                "\"verification_timeouts\":{},",
                "\"verification_mismatches\":{},",
                "\"rejected_total\":{},",
                "\"reject_counts\":{}",
                "}},",
                "\"latency\":{{",
                "\"samples\":{},",
                "\"total_elapsed_max_ms\":{},",
                "\"stage_deltas_ms\":{}",
                "}},",
                "\"artifacts\":{}",
                "}}"
            ),
            self.snapshot.render_json(),
            self.metrics.submitted(),
            self.metrics.verified_total(),
            self.metrics.verification_timeouts(),
            self.metrics.verification_mismatches(),
            self.metrics.rejected_total(),
            reject_counts,
            self.latency.samples(),
            self.latency.total_elapsed_max_ms(),
            stage_deltas,
            artifact_paths,
        )
    }
}

fn render_string_u64_map<'a>(entries: impl IntoIterator<Item = (&'a str, u64)>) -> String {
    let fields = entries
        .into_iter()
        .map(|(key, value)| format!("\"{}\":{}", escape_json(key), value))
        .collect::<Vec<_>>();
    format!("{{{}}}", fields.join(","))
}

fn render_optional_path_map<'a>(entries: impl IntoIterator<Item = (&'a str, Option<&'a PathBuf>)>) -> String {
    let fields = entries
        .into_iter()
        .map(|(key, value)| match value {
            Some(path) => format!(
                "\"{}\":\"{}\"",
                escape_json(key),
                escape_json(&path.display().to_string())
            ),
            None => format!("\"{}\":null", escape_json(key)),
        })
        .collect::<Vec<_>>();
    format!("{{{}}}", fields.join(","))
}

fn stage_label(stage: Stage) -> &'static str {
    match stage {
        Stage::ActivityObserved => "activity_observed",
        Stage::PositionsReconciled => "positions_reconciled",
        Stage::MarketQuoted => "market_quoted",
        Stage::PreTradeValidated => "pre_trade_validated",
        Stage::OrderSubmitted => "order_submitted",
        Stage::VerificationObserved => "verification_observed",
    }
}
