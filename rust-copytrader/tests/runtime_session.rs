use rust_copytrader::app::{RuntimeSession, SessionOutcome};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::{ReplayFixture, ReplayVerificationFrame};

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
