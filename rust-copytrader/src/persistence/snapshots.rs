use std::collections::VecDeque;
use std::fs;
use std::io;
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

pub fn session_snapshot_archive_path(root: &Path, session_id: &str, sequence: u64) -> PathBuf {
    root.join("sessions")
        .join(session_id)
        .join("snapshots")
        .join(format!("runtime-snapshot-{sequence:04}.json"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotWriteResult {
    pub latest_path: PathBuf,
    pub archive_path: PathBuf,
    pub pruned_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshotWriter {
    root: PathBuf,
    session_id: String,
    max_archives: usize,
    next_sequence: u64,
    retained_archives: VecDeque<PathBuf>,
}

impl SessionSnapshotWriter {
    pub fn new(root: impl Into<PathBuf>, session_id: impl Into<String>, max_archives: usize) -> Self {
        Self {
            root: root.into(),
            session_id: session_id.into(),
            max_archives: max_archives.max(1),
            next_sequence: 1,
            retained_archives: VecDeque::new(),
        }
    }

    pub fn persist(&mut self, snapshot: &SnapshotBundle) -> io::Result<SnapshotWriteResult> {
        let latest_path = session_snapshot_path(&self.root, &self.session_id);
        let archive_path = session_snapshot_archive_path(&self.root, &self.session_id, self.next_sequence);
        let payload = snapshot.render_json();

        write_file(&latest_path, &payload)?;
        write_file(&archive_path, &payload)?;

        self.next_sequence += 1;
        self.retained_archives.push_back(archive_path.clone());

        let mut pruned_paths = Vec::new();
        while self.retained_archives.len() > self.max_archives {
            if let Some(stale_path) = self.retained_archives.pop_front() {
                if stale_path.exists() {
                    fs::remove_file(&stale_path)?;
                }
                pruned_paths.push(stale_path);
            }
        }

        Ok(SnapshotWriteResult {
            latest_path,
            archive_path,
            pruned_paths,
        })
    }
}

fn write_file(path: &Path, payload: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, payload)
}

fn opt_json(value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
