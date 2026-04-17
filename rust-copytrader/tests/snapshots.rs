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
            selected_leader_wallet: Some("0xleader-wallet".into()),
            selected_leader_source: Some("file:.omx/discovery/selected-leader.env".into()),
            selected_leader_rank: Some("1".into()),
            selected_leader_pnl: Some("123.45".into()),
            selected_leader_username: Some("alpha".into()),
            selected_leader_review_status: Some("stable".into()),
            selected_leader_review_reasons: Some("none".into()),
            selected_leader_core_pool_count: Some("3".into()),
            selected_leader_core_pool_wallets: Some("0xaaa:95,0xbbb:88".into()),
            selected_leader_active_pool_count: Some("2".into()),
            selected_leader_active_pool_wallets: Some("0xaaa:95".into()),
            selected_leader_latest_activity_timestamp: Some("1776303488".into()),
            selected_leader_latest_activity_side: Some("BUY".into()),
            selected_leader_latest_activity_slug: Some("market-slug".into()),
            selected_leader_latest_activity_tx: Some("0xfeed".into()),
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
    assert!(json.contains("\"selected_leader_wallet\":\"0xleader-wallet\""));
    assert!(
        json.contains("\"selected_leader_source\":\"file:.omx/discovery/selected-leader.env\"")
    );
    assert!(json.contains("\"selected_leader_rank\":\"1\""));
    assert!(json.contains("\"selected_leader_pnl\":\"123.45\""));
    assert!(json.contains("\"selected_leader_username\":\"alpha\""));
    assert!(json.contains("\"selected_leader_review_status\":\"stable\""));
    assert!(json.contains("\"selected_leader_core_pool_count\":\"3\""));
    assert!(json.contains("\"selected_leader_latest_activity_side\":\"BUY\""));
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
            selected_leader_wallet: None,
            selected_leader_source: None,
            selected_leader_rank: None,
            selected_leader_pnl: None,
            selected_leader_username: None,
            selected_leader_review_status: None,
            selected_leader_review_reasons: None,
            selected_leader_core_pool_count: None,
            selected_leader_core_pool_wallets: None,
            selected_leader_active_pool_count: None,
            selected_leader_active_pool_wallets: None,
            selected_leader_latest_activity_timestamp: None,
            selected_leader_latest_activity_side: None,
            selected_leader_latest_activity_slug: None,
            selected_leader_latest_activity_tx: None,
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
    assert!(json.contains("\"selected_leader_wallet\":null"));
    assert!(json.contains("\"selected_leader_source\":null"));
    assert!(json.contains("\"selected_leader_rank\":null"));
    assert!(json.contains("\"selected_leader_pnl\":null"));
    assert!(json.contains("\"last_submit_status\":\"rejected:\\\\\\\"quote_stale\""));
    assert!(json.contains("\"last_correlation_id\":null"));
    assert!(json.contains("\"last_reject_reason\":null"));
    assert!(json.contains("\"last_stage\":null"));
}
