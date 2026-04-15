use rust_copytrader::adapters::transport::{
    ActivityTransport, MarketTransport, PositionsTransport, VerificationTransport,
    select_transport_boundary,
};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::ReplayFixture;

#[test]
fn transport_selector_preserves_replay_parity_and_live_gate() {
    let fixture = ReplayFixture::success_buy_follow();

    let blocked = select_transport_boundary(
        ActivityMode::LiveListen,
        LiveModeGate::for_mode(ActivityMode::LiveListen),
        &fixture,
    )
    .expect_err("live listen should stay blocked until the gate unlocks");
    assert_eq!(blocked, "activity_source_unverified");

    let replay = select_transport_boundary(
        ActivityMode::Replay,
        LiveModeGate::for_mode(ActivityMode::Replay),
        &fixture,
    )
    .expect("replay should be selectable");
    let shadow = select_transport_boundary(
        ActivityMode::ShadowPoll,
        LiveModeGate::for_mode(ActivityMode::ShadowPoll),
        &fixture,
    )
    .expect("shadow poll should be selectable");

    assert_eq!(replay.transport_name(), "replay");
    assert_eq!(shadow.transport_name(), "shadow_poll");
    assert_eq!(
        replay.read_activity().transaction_hash,
        shadow.read_activity().transaction_hash
    );
    assert_eq!(
        replay.read_positions().current.current_size,
        shadow.read_positions().current.current_size
    );
    assert_eq!(
        replay.read_market_quote().observed_at_ms,
        shadow.read_market_quote().observed_at_ms
    );
    assert_eq!(
        replay.read_verification("corr-success").observed_at_ms,
        shadow.read_verification("corr-success").observed_at_ms
    );

    let mut unlocked_gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    unlocked_gate.activity_source_verified = true;
    unlocked_gate.activity_source_under_budget = true;
    unlocked_gate.activity_capability_detected = true;
    unlocked_gate.positions_under_budget = true;
    unlocked_gate.execution_surface_ready = true;

    let live = select_transport_boundary(ActivityMode::LiveListen, unlocked_gate, &fixture)
        .expect("unlocked live listen should be selectable");
    assert_eq!(live.transport_name(), "live_listen");
    assert_eq!(live.read_activity().transaction_hash, "0xtx-success");
    assert_eq!(live.read_positions().reconciled_at_ms, 1_020);
    assert_eq!(live.read_market_quote().observed_at_ms, 1_028);
    assert_eq!(live.read_verification("corr-success").observed_at_ms, 1_082);
}
