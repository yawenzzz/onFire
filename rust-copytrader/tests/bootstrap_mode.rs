use rust_copytrader::app::{BootstrapDecision, RuntimeBootstrap};
use rust_copytrader::config::{ActivityMode, LiveModeGate};

#[test]
fn bootstrap_blocks_live_mode_with_explicit_reason() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let decision = RuntimeBootstrap::new(ActivityMode::LiveListen, gate).decide();

    assert_eq!(
        decision,
        BootstrapDecision::Blocked("activity_source_unverified".into())
    );
}

#[test]
fn bootstrap_allows_shadow_poll_without_live_unlock() {
    let gate = LiveModeGate::for_mode(ActivityMode::ShadowPoll);
    let decision = RuntimeBootstrap::new(ActivityMode::ShadowPoll, gate).decide();

    assert_eq!(decision, BootstrapDecision::ShadowPoll);
}

#[test]
fn bootstrap_allows_replay_without_live_unlock() {
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let decision = RuntimeBootstrap::new(ActivityMode::Replay, gate).decide();

    assert_eq!(decision, BootstrapDecision::Replay);
}

#[test]
fn bootstrap_allows_live_mode_once_all_gates_are_green() {
    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = true;
    gate.activity_source_under_budget = true;
    gate.activity_capability_detected = true;
    gate.positions_under_budget = true;
    gate.execution_surface_ready = true;

    let decision = RuntimeBootstrap::new(ActivityMode::LiveListen, gate).decide();

    assert_eq!(decision, BootstrapDecision::LiveListen);
}
