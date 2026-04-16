use rust_copytrader::app::{RuntimeBootstrap, RuntimeSession, SessionOutcome};
use rust_copytrader::config::{
    ActivityMode, CommandAdapterConfig, ExecutionAdapterConfig, LiveModeGate, SigningAdapterConfig,
    SubmitAdapterConfig,
};
use rust_copytrader::replay::fixture::{ReplayFixture, ReplayVerificationFrame};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn blocked_live_session_reports_reason_without_processing_fixture() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let mut session = RuntimeSession::new(ActivityMode::LiveListen, gate);
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_replay(&fixture);
    let snapshot = session.snapshot().expect("blocked snapshot expected");

    assert_eq!(
        outcome,
        SessionOutcome::Blocked("activity_source_unverified".into())
    );
    assert_eq!(session.metrics().submitted(), 0);
    assert_eq!(session.metrics().rejected_total(), 0);
    assert_eq!(snapshot.runtime.mode, "blocked");
    assert_eq!(
        snapshot.runtime.blocked_reason.as_deref(),
        Some("activity_source_unverified")
    );
    assert!(!snapshot.runtime.live_mode_unlocked);
    assert_eq!(
        snapshot.runtime.last_submit_status,
        "blocked:activity_source_unverified"
    );
    assert_eq!(snapshot.runtime.verification_pending, 0);
    assert_eq!(snapshot.leader.leader_id, "leader-1");
    assert_eq!(snapshot.leader.last_transaction_hash, "0xtx-success");
}

#[test]
fn replay_session_accumulates_submit_metrics_and_snapshot_after_success() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_replay(&fixture);
    let snapshot = session.snapshot().expect("snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(session.metrics().submitted(), 1);
    assert_eq!(session.metrics().verified_total(), 1);
    assert_eq!(session.metrics().rejected_total(), 0);
    assert_eq!(snapshot.runtime.mode, "replay");
    assert_eq!(snapshot.runtime.blocked_reason, None);
    assert_eq!(snapshot.runtime.last_submit_status, "verified");
    assert_eq!(
        snapshot.runtime.last_correlation_id.as_deref(),
        Some("corr-success")
    );
    assert_eq!(snapshot.runtime.last_reject_reason, None);
    assert_eq!(
        snapshot.runtime.last_stage.as_deref(),
        Some("verification_observed")
    );
    assert_eq!(snapshot.runtime.last_total_elapsed_ms, 82);
    assert_eq!(snapshot.runtime.verification_pending, 0);
    assert_eq!(snapshot.leader.last_position_size, 14);
}

#[test]
fn replay_session_tracks_reject_reason_and_preserves_latest_leader_snapshot() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.current_position.current_size = fixture.previous_position.current_size;

    let outcome = session.process_replay(&fixture);
    let snapshot = session.snapshot().expect("snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(session.metrics().submitted(), 0);
    assert_eq!(session.metrics().rejected_total(), 1);
    assert_eq!(session.metrics().reject_count("no_net_position_change"), 1);
    assert_eq!(
        snapshot.runtime.last_submit_status,
        "rejected:no_net_position_change"
    );
    assert_eq!(
        snapshot.runtime.last_correlation_id.as_deref(),
        Some("0xtx-success")
    );
    assert_eq!(
        snapshot.runtime.last_reject_reason.as_deref(),
        Some("no_net_position_change")
    );
    assert_eq!(
        snapshot.runtime.last_stage.as_deref(),
        Some("activity_observed")
    );
    assert_eq!(snapshot.runtime.last_total_elapsed_ms, 0);
    assert_eq!(snapshot.leader.last_position_size, 10);
}

#[test]
fn replay_session_distinguishes_preview_rejections_from_submit_failures() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut preview_session = RuntimeSession::new(ActivityMode::Replay, gate.clone());
    let mut preview_fixture = ReplayFixture::success_buy_follow();
    preview_fixture.preview = rust_copytrader::replay::fixture::ReplayPreviewResult::Rejected;

    let preview_outcome = preview_session.process_replay(&preview_fixture);
    let preview_snapshot = preview_session
        .snapshot()
        .expect("preview snapshot expected");

    assert_eq!(preview_outcome, SessionOutcome::Processed);
    assert_eq!(preview_session.metrics().submitted(), 0);
    assert_eq!(preview_session.metrics().rejected_total(), 1);
    assert_eq!(
        preview_session.metrics().reject_count("preview_rejected"),
        1
    );
    assert_eq!(
        preview_snapshot.runtime.last_submit_status,
        "preview_rejected"
    );

    let mut submit_session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut submit_fixture = ReplayFixture::success_buy_follow();
    submit_fixture.submit = rust_copytrader::replay::fixture::ReplaySubmitResult::Rejected;

    let submit_outcome = submit_session.process_replay(&submit_fixture);
    let submit_snapshot = submit_session.snapshot().expect("submit snapshot expected");

    assert_eq!(submit_outcome, SessionOutcome::Processed);
    assert_eq!(submit_session.metrics().submitted(), 0);
    assert_eq!(submit_session.metrics().rejected_total(), 1);
    assert_eq!(submit_session.metrics().reject_count("submit_rejected"), 1);
    assert_eq!(
        submit_snapshot.runtime.last_submit_status,
        "submit_rejected"
    );
}

#[test]
fn shadow_poll_session_reports_runtime_mode_without_live_unlock() {
    let gate = LiveModeGate::for_mode(ActivityMode::ShadowPoll);
    let mut session = RuntimeSession::new(ActivityMode::ShadowPoll, gate);
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_replay(&fixture);
    let snapshot = session.snapshot().expect("shadow-poll snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(snapshot.runtime.mode, "shadow_poll");
    assert!(!snapshot.runtime.live_mode_unlocked);
    assert_eq!(snapshot.runtime.blocked_reason, None);
    assert_eq!(snapshot.runtime.last_submit_status, "verified");
}

#[test]
fn live_session_processes_fixture_through_unified_transport_when_gate_is_unlocked() {
    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = true;
    gate.activity_source_under_budget = true;
    gate.activity_capability_detected = true;
    gate.positions_under_budget = true;
    gate.execution_surface_ready = true;

    let mut session = RuntimeSession::new(ActivityMode::LiveListen, gate);
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_fixture(&fixture);
    let snapshot = session.snapshot().expect("live snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(snapshot.runtime.mode, "live_listen");
    assert!(snapshot.runtime.live_mode_unlocked);
    assert_eq!(snapshot.runtime.blocked_reason, None);
    assert_eq!(snapshot.runtime.last_submit_status, "verified");
    assert_eq!(
        snapshot.runtime.last_stage.as_deref(),
        Some("verification_observed")
    );
}

#[test]
fn config_driven_live_session_stays_blocked_with_default_execution_selection() {
    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = true;
    gate.activity_source_under_budget = true;
    gate.activity_capability_detected = true;
    gate.positions_under_budget = true;
    gate.execution_surface_ready = true;

    let mut session = RuntimeSession::with_execution_config(
        ActivityMode::LiveListen,
        gate,
        ExecutionAdapterConfig::default(),
    );
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_fixture(&fixture);
    let snapshot = session.snapshot().expect("blocked snapshot expected");

    assert_eq!(
        outcome,
        SessionOutcome::Blocked("execution_surface_not_ready".into())
    );
    assert_eq!(snapshot.runtime.mode, "blocked");
    assert!(!snapshot.runtime.live_mode_unlocked);
    assert_eq!(
        snapshot.runtime.blocked_reason.as_deref(),
        Some("execution_surface_not_ready")
    );
}

#[test]
fn config_driven_live_session_processes_fixture_with_command_signing_and_http_submit() {
    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = true;
    gate.activity_source_under_budget = true;
    gate.activity_capability_detected = true;
    gate.positions_under_budget = true;
    gate.execution_surface_ready = true;

    let mut session = RuntimeSession::with_execution_config(
        ActivityMode::LiveListen,
        gate,
        ExecutionAdapterConfig::live_command_http("python3", "https://clob.polymarket.com", "curl"),
    );
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_fixture(&fixture);
    let snapshot = session.snapshot().expect("live snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(snapshot.runtime.mode, "live_listen");
    assert!(snapshot.runtime.live_mode_unlocked);
    assert_eq!(snapshot.runtime.blocked_reason, None);
    assert_eq!(snapshot.runtime.last_submit_status, "verified");
}

#[test]
fn runtime_bootstrap_exposes_command_execution_wiring_for_live_ready_pair() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let bootstrap = RuntimeBootstrap::with_execution_config(
        ActivityMode::LiveListen,
        gate,
        ExecutionAdapterConfig {
            signing: SigningAdapterConfig::command_with_args(
                "python3",
                vec!["scripts/sign_order.py".into(), "--json".into()],
            ),
            submit: SubmitAdapterConfig::http_with_command(
                "https://clob.polymarket.com",
                CommandAdapterConfig::new("curl"),
            ),
        },
    );

    let wiring = bootstrap
        .live_execution_wiring()
        .expect("live execution wiring expected");

    assert_eq!(wiring.signing.program, "python3");
    assert_eq!(
        wiring.signing.args,
        vec!["scripts/sign_order.py".to_string(), "--json".to_string()]
    );
    assert_eq!(wiring.submit.program, "curl");
    assert!(wiring.submit.args.is_empty());
    assert_eq!(wiring.submit_base_url, "https://clob.polymarket.com");
    assert_eq!(wiring.submit_connect_timeout_ms, 50);
    assert_eq!(wiring.submit_max_time_ms, 200);
}

#[test]
fn runtime_bootstrap_exposes_repo_local_helper_bridge_for_live_selection() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let bootstrap = RuntimeBootstrap::with_execution_config(
        ActivityMode::LiveListen,
        gate,
        ExecutionAdapterConfig::live_command_helper_http(
            "python3",
            "https://clob.polymarket.com",
            "curl",
        ),
    );

    let wiring = bootstrap
        .live_execution_wiring()
        .expect("live execution wiring expected");
    let l2_helper = bootstrap
        .live_l2_header_helper()
        .expect("repo-local l2 helper expected");

    assert_eq!(wiring.signing.program, "python3");
    assert_eq!(
        wiring.signing.args,
        vec!["scripts/sign_order.py".to_string(), "--json".to_string()]
    );
    assert_eq!(l2_helper.program, "python3");
    assert_eq!(
        l2_helper.args,
        vec!["scripts/sign_l2.py".to_string(), "--json".to_string()]
    );
    assert_eq!(wiring.submit.program, "curl");
}

#[test]
fn runtime_bootstrap_loads_live_execution_wiring_from_root_without_unlocking_live_mode() {
    let root = unique_temp_root("runtime-bootstrap-root");
    fs::create_dir_all(&root).expect("temp root created");
    fs::write(
        root.join(".env.local"),
        concat!(
            "RUST_COPYTRADER_SIGNING_PROGRAM=python3\n",
            "RUST_COPYTRADER_SUBMIT_PROGRAM=curl\n",
            "CLOB_HOST=https://clob.polymarket.com\n",
            "RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS=75\n",
            "RUST_COPYTRADER_SUBMIT_MAX_TIME_MS=150\n",
        ),
    )
    .expect(".env.local written");

    let bootstrap = RuntimeBootstrap::from_root(
        ActivityMode::LiveListen,
        LiveModeGate::for_mode(ActivityMode::LiveListen),
        &root,
    )
    .expect("bootstrap from root");

    assert_eq!(
        bootstrap.decide(),
        rust_copytrader::app::BootstrapDecision::Blocked("activity_source_unverified".into())
    );

    let wiring = bootstrap
        .live_execution_wiring()
        .expect("loaded live execution wiring");
    assert_eq!(wiring.signing.program, "python3");
    assert_eq!(
        wiring.signing.args,
        vec!["scripts/sign_order.py".to_string(), "--json".to_string()]
    );
    assert_eq!(wiring.submit.program, "curl");
    assert_eq!(wiring.submit_base_url, "https://clob.polymarket.com");
    assert_eq!(wiring.submit_connect_timeout_ms, 75);
    assert_eq!(wiring.submit_max_time_ms, 150);
    assert_eq!(
        bootstrap
            .live_l2_header_helper()
            .expect("l2 helper wiring expected")
            .args,
        vec!["scripts/sign_l2.py".to_string(), "--json".to_string()]
    );

    fs::remove_dir_all(root).expect("temp root removed");
}

#[test]
fn replay_session_tracks_verification_timeout_metrics_and_snapshot_state() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.verification = ReplayVerificationFrame::Timeout {
        observed_at_ms: 1_140,
    };

    let outcome = session.process_replay(&fixture);
    let snapshot = session.snapshot().expect("timeout snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(session.metrics().submitted(), 1);
    assert_eq!(session.metrics().verified_total(), 0);
    assert_eq!(session.metrics().verification_timeouts(), 1);
    assert_eq!(session.metrics().verification_mismatches(), 0);
    assert_eq!(snapshot.runtime.last_submit_status, "verification_timeout");
    assert_eq!(snapshot.runtime.verification_pending, 0);
    assert_eq!(
        snapshot.runtime.last_correlation_id.as_deref(),
        Some("corr-success")
    );
    assert_eq!(
        snapshot.runtime.last_stage.as_deref(),
        Some("verification_observed")
    );
    assert_eq!(snapshot.runtime.last_total_elapsed_ms, 140);
}

#[test]
fn replay_session_tracks_verification_mismatch_metrics_and_snapshot_state() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session = RuntimeSession::new(ActivityMode::Replay, gate);
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.verification = ReplayVerificationFrame::Mismatch {
        observed_at_ms: 1_090,
    };

    let outcome = session.process_replay(&fixture);
    let snapshot = session
        .snapshot()
        .expect("verification-mismatch snapshot expected");

    assert_eq!(outcome, SessionOutcome::Processed);
    assert_eq!(session.metrics().submitted(), 1);
    assert_eq!(session.metrics().verified_total(), 0);
    assert_eq!(session.metrics().verification_timeouts(), 0);
    assert_eq!(session.metrics().verification_mismatches(), 1);
    assert_eq!(snapshot.runtime.last_submit_status, "verification_mismatch");
    assert_eq!(snapshot.runtime.verification_pending, 0);
    assert_eq!(
        snapshot.runtime.last_correlation_id.as_deref(),
        Some("corr-success")
    );
    assert_eq!(
        snapshot.runtime.last_stage.as_deref(),
        Some("verification_observed")
    );
    assert_eq!(snapshot.runtime.last_total_elapsed_ms, 90);
}

fn unique_temp_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rust-copytrader-{name}-{suffix}"))
}
