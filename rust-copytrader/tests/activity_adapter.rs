use rust_copytrader::adapters::activity::{ActivityAdapterMode, ActivitySourceSelector};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::domain::events::ActivityEvent;

#[test]
fn live_mode_is_rejected_until_gate_is_unlocked() {
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let selector = ActivitySourceSelector::new(ActivityMode::LiveListen, gate);

    assert_eq!(selector.resolve_mode(), ActivityAdapterMode::Blocked);
    assert_eq!(
        selector.blocked_reason().as_deref(),
        Some("activity_source_unverified")
    );
}

#[test]
fn shadow_poll_mode_remains_available_when_live_mode_is_blocked() {
    let gate = LiveModeGate::for_mode(ActivityMode::ShadowPoll);
    let selector = ActivitySourceSelector::new(ActivityMode::ShadowPoll, gate);

    assert_eq!(selector.resolve_mode(), ActivityAdapterMode::ShadowPoll);
    assert_eq!(selector.blocked_reason(), None);
}

#[test]
fn normalized_activity_event_exposes_proxy_wallet_and_transaction() {
    let event = ActivityEvent::new("leader-1", "0xtx", "BUY", "asset-9", 42, 1_234_567);

    assert_eq!(event.proxy_wallet, "leader-1");
    assert_eq!(event.transaction_hash, "0xtx");
    assert_eq!(event.side, "BUY");
    assert_eq!(event.asset_id, "asset-9");
    assert_eq!(event.size, 42);
    assert_eq!(event.observed_at_ms, 1_234_567);
}
