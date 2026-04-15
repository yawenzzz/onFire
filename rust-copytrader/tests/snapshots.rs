use rust_copytrader::persistence::snapshots::{
    LeaderStateSnapshot, RuntimeSnapshot, SnapshotBundle, session_snapshot_path,
};
use std::path::Path;

#[test]
fn session_snapshot_path_is_local_file_only_under_session_root() {
    let path = session_snapshot_path(Path::new("/tmp/state-root"), "session-7");
    assert_eq!(
        path,
        Path::new("/tmp/state-root/sessions/session-7/runtime-snapshot.json")
    );
}

#[test]
fn snapshot_bundle_renders_stable_json_shape() {
    let snapshot = SnapshotBundle {
        leader: LeaderStateSnapshot {
            leader_id: "leader-1".into(),
            last_activity_at_ms: 1_000,
            last_transaction_hash: "0xtx".into(),
            last_position_size: 14,
        },
        runtime: RuntimeSnapshot {
            mode: "shadow_poll".into(),
            live_mode_unlocked: false,
            blocked_reason: Some("activity_source_unverified".into()),
            verification_pending: 2,
            last_submit_status: "submitted_unverified".into(),
        },
    };

    let json = snapshot.render_json();

    assert!(json.contains("\"leader_id\":\"leader-1\""));
    assert!(json.contains("\"last_transaction_hash\":\"0xtx\""));
    assert!(json.contains("\"mode\":\"shadow_poll\""));
    assert!(json.contains("\"blocked_reason\":\"activity_source_unverified\""));
    assert!(json.contains("\"verification_pending\":2"));
}
