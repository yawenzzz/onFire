use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderStateSnapshot {
    pub leader_id: String,
    pub last_activity_at_ms: u64,
    pub last_transaction_hash: String,
    pub last_position_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub mode: String,
    pub live_mode_unlocked: bool,
    pub blocked_reason: Option<String>,
    pub verification_pending: u64,
    pub last_submit_status: String,
    pub last_correlation_id: Option<String>,
    pub last_reject_reason: Option<String>,
    pub last_stage: Option<String>,
    pub last_total_elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBundle {
    pub leader: LeaderStateSnapshot,
    pub runtime: RuntimeSnapshot,
}

impl SnapshotBundle {
    pub fn render_json(&self) -> String {
        let blocked_reason = opt_json(&self.runtime.blocked_reason);
        let last_correlation_id = opt_json(&self.runtime.last_correlation_id);
        let last_reject_reason = opt_json(&self.runtime.last_reject_reason);
        let last_stage = opt_json(&self.runtime.last_stage);
        format!(
            concat!(
                "{{",
                "\"leader\":{{",
                "\"leader_id\":\"{}\",",
                "\"last_activity_at_ms\":{},",
                "\"last_transaction_hash\":\"{}\",",
                "\"last_position_size\":{}",
                "}},",
                "\"runtime\":{{",
                "\"mode\":\"{}\",",
                "\"live_mode_unlocked\":{},",
                "\"blocked_reason\":{},",
                "\"verification_pending\":{},",
                "\"last_submit_status\":\"{}\",",
                "\"last_correlation_id\":{},",
                "\"last_reject_reason\":{},",
                "\"last_stage\":{},",
                "\"last_total_elapsed_ms\":{}",
                "}}",
                "}}"
            ),
            escape_json(&self.leader.leader_id),
            self.leader.last_activity_at_ms,
            escape_json(&self.leader.last_transaction_hash),
            self.leader.last_position_size,
            escape_json(&self.runtime.mode),
            self.runtime.live_mode_unlocked,
            blocked_reason,
            self.runtime.verification_pending,
            escape_json(&self.runtime.last_submit_status),
            last_correlation_id,
            last_reject_reason,
            last_stage,
            self.runtime.last_total_elapsed_ms,
        )
    }
}

pub fn session_snapshot_path(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions")
        .join(session_id)
        .join("runtime-snapshot.json")
}

fn opt_json(value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
