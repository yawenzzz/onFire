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
            last_correlation_id: Some("corr-7".into()),
            last_reject_reason: Some("quote_stale".into()),
            last_stage: Some("market_quoted".into()),
            last_total_elapsed_ms: 94,
        },
    };

    let json = snapshot.render_json();

    assert!(json.contains("\"leader_id\":\"leader-1\""));
    assert!(json.contains("\"last_transaction_hash\":\"0xtx\""));
    assert!(json.contains("\"mode\":\"shadow_poll\""));
    assert!(json.contains("\"blocked_reason\":\"activity_source_unverified\""));
    assert!(json.contains("\"verification_pending\":2"));
    assert!(json.contains("\"last_correlation_id\":\"corr-7\""));
    assert!(json.contains("\"last_reject_reason\":\"quote_stale\""));
    assert!(json.contains("\"last_stage\":\"market_quoted\""));
    assert!(json.contains("\"last_total_elapsed_ms\":94"));
}

#[test]
fn snapshot_bundle_renders_nulls_and_escaped_strings_for_operator_reports() {
    let snapshot = SnapshotBundle {
        leader: LeaderStateSnapshot {
            leader_id: "leader-\\\"1".into(),
            last_activity_at_ms: 2_000,
            last_transaction_hash: "0x\\\"tx".into(),
            last_position_size: -2,
        },
        runtime: RuntimeSnapshot {
            mode: "replay".into(),
            live_mode_unlocked: true,
            blocked_reason: None,
            verification_pending: 0,
            last_submit_status: "rejected:\\\"quote_stale".into(),
            last_correlation_id: None,
            last_reject_reason: None,
            last_stage: None,
            last_total_elapsed_ms: 0,
        },
    };

    let json = snapshot.render_json();

    assert!(json.contains("\"leader_id\":\"leader-\\\\\\\"1\""));
    assert!(json.contains("\"last_transaction_hash\":\"0x\\\\\\\"tx\""));
    assert!(json.contains("\"live_mode_unlocked\":true"));
    assert!(json.contains("\"blocked_reason\":null"));
    assert!(json.contains("\"last_submit_status\":\"rejected:\\\\\\\"quote_stale\""));
    assert!(json.contains("\"last_correlation_id\":null"));
    assert!(json.contains("\"last_reject_reason\":null"));
    assert!(json.contains("\"last_stage\":null"));
}
