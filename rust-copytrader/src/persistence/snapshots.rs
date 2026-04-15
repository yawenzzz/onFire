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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBundle {
    pub leader: LeaderStateSnapshot,
    pub runtime: RuntimeSnapshot,
}

impl SnapshotBundle {
    pub fn render_json(&self) -> String {
        let blocked_reason = self
            .runtime
            .blocked_reason
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .unwrap_or_else(|| "null".to_string());
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
                "\"last_submit_status\":\"{}\"",
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
        )
    }
}

pub fn session_snapshot_path(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions")
        .join(session_id)
        .join("runtime-snapshot.json")
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
