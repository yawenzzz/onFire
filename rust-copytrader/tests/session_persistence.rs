use rust_copytrader::app::{RuntimeSession, RuntimeSessionRecorder};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::{ReplayFixture, ReplayVerificationFrame};
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
    let first = recorder
        .persist(&session)
        .expect("first persist should work");

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
    assert!(summary.contains("selected_leader_wallet=none"));
    assert!(summary.contains("selected_leader_source=none"));
    assert!(summary.contains("last_submit_status=verified"));
    assert!(summary.contains("submitted=1"));

    let report = fs::read_to_string(&first.report_path).expect("report should be readable");
    assert!(report.contains("\"mode\":\"replay\""));
    assert!(report.contains("\"submitted\":1"));
    assert!(report.contains("\"verified_total\":1"));
    assert!(report.contains("\"verification_observed\":22"));

    session.process_replay(&ReplayFixture::success_buy_follow());
    let second = recorder
        .persist(&session)
        .expect("second persist should work");

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
    assert_eq!(
        archive_count, 1,
        "snapshot retention should prune old archives"
    );

    fs::remove_dir_all(root).expect("temp artifacts should be removed");
}

#[test]
fn runtime_session_recorder_refreshes_latest_snapshot_and_report_across_rotations() {
    let root = unique_temp_root("runtime-session-recorder-refresh");
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut recorder = RuntimeSessionRecorder::new(&root, "session-8", 2, 1);

    session.process_replay(&ReplayFixture::success_buy_follow());
    let first = recorder
        .persist(&session)
        .expect("first persist should work");

    let mut timeout_fixture = ReplayFixture::success_buy_follow();
    timeout_fixture.verification = ReplayVerificationFrame::Timeout {
        observed_at_ms: 1_140,
    };
    session.process_replay(&timeout_fixture);
    let second = recorder
        .persist(&session)
        .expect("second persist should work");

    let mut mismatch_fixture = ReplayFixture::success_buy_follow();
    mismatch_fixture.verification = ReplayVerificationFrame::Mismatch {
        observed_at_ms: 1_090,
    };
    session.process_replay(&mismatch_fixture);
    let third = recorder
        .persist(&session)
        .expect("third persist should work");

    assert!(
        first
            .snapshot_archive_path
            .ends_with("sessions/session-8/snapshots/runtime-snapshot-0001.json")
    );
    assert!(
        second
            .snapshot_archive_path
            .ends_with("sessions/session-8/snapshots/runtime-snapshot-0002.json")
    );
    assert!(
        third
            .snapshot_archive_path
            .ends_with("sessions/session-8/snapshots/runtime-snapshot-0003.json")
    );
    assert!(
        !first.snapshot_archive_path.exists(),
        "oldest archive should be pruned once retention is exceeded"
    );
    assert!(second.snapshot_archive_path.exists());
    assert!(third.snapshot_archive_path.exists());
    assert!(
        third
            .activity_log_path
            .ends_with("sessions/session-8/logs/activity-0003.jsonl")
    );
    assert!(
        third
            .order_log_path
            .ends_with("sessions/session-8/logs/orders-0003.jsonl")
    );
    assert!(
        third
            .verification_log_path
            .ends_with("sessions/session-8/logs/verification-0003.jsonl")
    );

    let latest_snapshot =
        fs::read_to_string(&third.latest_snapshot_path).expect("latest snapshot should exist");
    let latest_archive =
        fs::read_to_string(&third.snapshot_archive_path).expect("latest archive should exist");
    assert_eq!(
        latest_snapshot, latest_archive,
        "latest snapshot should mirror the newest archive payload"
    );
    assert!(latest_snapshot.contains("\"last_submit_status\":\"verification_mismatch\""));
    assert!(latest_snapshot.contains("\"last_total_elapsed_ms\":90"));

    let report = fs::read_to_string(&third.report_path).expect("report should be readable");
    assert!(report.contains("\"last_submit_status\":\"verification_mismatch\""));
    assert!(report.contains("\"submitted\":3"));
    assert!(report.contains("\"verified_total\":1"));
    assert!(report.contains("\"verification_timeouts\":1"));
    assert!(report.contains("\"verification_mismatches\":1"));
    assert!(report.contains("\"samples\":3"));
    assert!(report.contains("\"total_elapsed_max_ms\":140"));
    assert!(report.contains(&format!(
        "\"latest_snapshot_path\":\"{}\"",
        third.latest_snapshot_path.display()
    )));
    assert!(report.contains(&format!(
        "\"snapshot_archive_path\":\"{}\"",
        third.snapshot_archive_path.display()
    )));
    assert!(report.contains(&format!(
        "\"activity_log_path\":\"{}\"",
        third.activity_log_path.display()
    )));
    assert!(report.contains(&format!(
        "\"order_log_path\":\"{}\"",
        third.order_log_path.display()
    )));
    assert!(report.contains(&format!(
        "\"verification_log_path\":\"{}\"",
        third.verification_log_path.display()
    )));

    let summary = fs::read_to_string(&third.summary_path).expect("summary should be readable");
    assert!(summary.contains("last_submit_status=verification_mismatch"));
    assert!(summary.contains("selected_leader_wallet=none"));
    assert!(summary.contains("selected_leader_source=none"));
    assert!(summary.contains("submitted=3"));
    assert!(summary.contains("verified_total=1"));
    assert!(summary.contains("rejected_total=0"));
    assert!(summary.contains("latency_samples=3"));
    assert!(summary.contains("max_total_elapsed_ms=140"));

    fs::remove_dir_all(root).expect("temp artifacts should be removed");
}

#[test]
fn runtime_session_recorder_writes_correlated_jsonl_records_for_operator_artifacts() {
    let root = unique_temp_root("runtime-session-recorder-jsonl");
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut recorder = RuntimeSessionRecorder::new(&root, "session-9", 1, 2);

    session.process_replay(&ReplayFixture::success_buy_follow());
    let persisted = recorder
        .persist(&session)
        .expect("persist should write operator artifacts");

    let activity_log =
        fs::read_to_string(&persisted.activity_log_path).expect("activity log should be readable");
    assert_eq!(
        activity_log.trim(),
        "{\"leader_id\":\"leader-1\",\"selected_leader_wallet\":null,\"selected_leader_source\":null,\"last_activity_at_ms\":1000,\"last_transaction_hash\":\"0xtx-success\"}"
    );

    let order_log =
        fs::read_to_string(&persisted.order_log_path).expect("order log should be readable");
    assert_eq!(
        order_log.trim(),
        "{\"selected_leader_wallet\":null,\"selected_leader_source\":null,\"last_submit_status\":\"verified\",\"last_correlation_id\":\"corr-success\",\"last_reject_reason\":null,\"last_stage\":\"verification_observed\",\"last_total_elapsed_ms\":82}"
    );

    let verification_log = fs::read_to_string(&persisted.verification_log_path)
        .expect("verification log should be readable");
    assert_eq!(
        verification_log.trim(),
        "{\"selected_leader_wallet\":null,\"selected_leader_source\":null,\"verification_pending\":0,\"last_submit_status\":\"verified\",\"last_correlation_id\":\"corr-success\"}"
    );

    fs::remove_dir_all(root).expect("temp artifacts should be removed");
}

#[test]
fn runtime_session_recorder_persists_selected_leader_metadata_in_logs_and_summary() {
    let root = unique_temp_root("runtime-session-recorder-selected-leader");
    fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir created");
    fs::write(
        root.join(".omx/discovery/selected-leader.env"),
        "COPYTRADER_DISCOVERY_WALLET=0xselected-leader\n",
    )
    .expect("selected leader env written");

    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session =
        RuntimeSession::from_root(ActivityMode::Replay, gate, &root).expect("session from root");
    let mut recorder = RuntimeSessionRecorder::new(&root, "session-10", 1, 2);

    session.process_replay(&ReplayFixture::success_buy_follow());
    let persisted = recorder
        .persist(&session)
        .expect("persist should write operator artifacts");

    let summary = fs::read_to_string(&persisted.summary_path).expect("summary should be readable");
    assert!(summary.contains("selected_leader_wallet=0xselected-leader"));
    assert!(summary.contains("selected_leader_source=file:.omx/discovery/selected-leader.env"));

    let activity_log =
        fs::read_to_string(&persisted.activity_log_path).expect("activity log should be readable");
    assert!(activity_log.contains("\"selected_leader_wallet\":\"0xselected-leader\""));
    assert!(
        activity_log
            .contains("\"selected_leader_source\":\"file:.omx/discovery/selected-leader.env\"")
    );

    let order_log =
        fs::read_to_string(&persisted.order_log_path).expect("order log should be readable");
    assert!(order_log.contains("\"selected_leader_wallet\":\"0xselected-leader\""));

    let verification_log = fs::read_to_string(&persisted.verification_log_path)
        .expect("verification log should be readable");
    assert!(verification_log.contains("\"selected_leader_wallet\":\"0xselected-leader\""));

    fs::remove_dir_all(root).expect("temp artifacts should be removed");
}
