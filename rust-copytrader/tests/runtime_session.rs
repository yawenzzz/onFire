use rust_copytrader::app::{RuntimeSession, SessionOutcome};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::ReplayFixture;

#[test]
fn blocked_live_session_reports_reason_without_processing_fixture() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let mut session = RuntimeSession::new(ActivityMode::LiveListen, gate);
    let fixture = ReplayFixture::success_buy_follow();

    let outcome = session.process_replay(&fixture);

    assert_eq!(
        outcome,
        SessionOutcome::Blocked("activity_source_unverified".into())
    );
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
    assert_eq!(snapshot.runtime.last_correlation_id.as_deref(), Some("corr-success"));
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
    assert_eq!(snapshot.runtime.last_correlation_id.as_deref(), Some("0xtx-success"));
    assert_eq!(
        snapshot.runtime.last_reject_reason.as_deref(),
        Some("no_net_position_change")
    );
    assert_eq!(snapshot.runtime.last_stage.as_deref(), Some("activity_observed"));
    assert_eq!(snapshot.runtime.last_total_elapsed_ms, 0);
    assert_eq!(snapshot.leader.last_position_size, 10);
}
