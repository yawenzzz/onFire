use rust_copytrader::app::{RuntimeSession, RuntimeSessionRecorder};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::ReplayFixture;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("rust-copytrader-{name}-{suffix}"))
}

#[test]
fn runtime_session_recorder_persists_reports_and_rotates_session_artifacts() {
    let root = unique_temp_root("runtime-session-recorder");
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut recorder = RuntimeSessionRecorder::new(&root, "session-7", 1, 1);

    session.process_replay(&ReplayFixture::success_buy_follow());
    let first = recorder.persist(&session).expect("first persist should work");

    assert!(first.latest_snapshot_path.exists());
    assert!(first.snapshot_archive_path.exists());
    assert!(first.activity_log_path.exists());
    assert!(first.order_log_path.exists());
    assert!(first.verification_log_path.exists());
    assert!(first.report_path.exists());
    assert!(first.summary_path.exists());
    assert!(
        first
            .activity_log_path
            .ends_with("sessions/session-7/logs/activity-0001.jsonl")
    );
    assert!(
        first
            .order_log_path
            .ends_with("sessions/session-7/logs/orders-0001.jsonl")
    );
    assert!(
        first
            .verification_log_path
            .ends_with("sessions/session-7/logs/verification-0001.jsonl")
    );

    let summary = fs::read_to_string(&first.summary_path).expect("summary should be readable");
    assert!(summary.contains("mode=replay"));
    assert!(summary.contains("last_submit_status=verified"));
    assert!(summary.contains("submitted=1"));

    let report = fs::read_to_string(&first.report_path).expect("report should be readable");
    assert!(report.contains("\"mode\":\"replay\""));
    assert!(report.contains("\"submitted\":1"));
    assert!(report.contains("\"verified_total\":1"));
    assert!(report.contains("\"verification_observed\":22"));

    session.process_replay(&ReplayFixture::success_buy_follow());
    let second = recorder.persist(&session).expect("second persist should work");

    assert!(
        second
            .activity_log_path
            .ends_with("sessions/session-7/logs/activity-0002.jsonl")
    );
    assert!(
        second
            .order_log_path
            .ends_with("sessions/session-7/logs/orders-0002.jsonl")
    );
    assert!(
        second
            .verification_log_path
            .ends_with("sessions/session-7/logs/verification-0002.jsonl")
    );

    let archive_dir = root.join("sessions/session-7/snapshots");
    let archive_count = fs::read_dir(&archive_dir)
        .expect("snapshot archive dir should exist")
        .count();
    assert_eq!(archive_count, 1, "snapshot retention should prune old archives");

    fs::remove_dir_all(root).expect("temp artifacts should be removed");
}
