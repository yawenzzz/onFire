use rust_copytrader::adapters::order_api::OrderSide;
use rust_copytrader::domain::events::ActivityEvent;
use rust_copytrader::pipeline::trace_context::Stage;
use rust_copytrader::replay::fixture::{
    MarketFixture, PositionFixtures, ReplayFixture, SubmitFixture, VerificationFixture,
};
use rust_copytrader::replay::harness::DeterministicReplayHarness;

#[test]
fn replay_harness_runs_the_fixed_path_through_verified_submit() {
    let fixture = ReplayFixture {
        activity: ActivityEvent::new("leader-1", "0xtx", "BUY", "asset-9", 4, 1_000),
        positions: PositionFixtures::new(
            ("leader-1", "asset-9", 10, 1_000, 4),
            ("leader-1", "asset-9", 14, 1_035, 6),
        ),
        market: MarketFixture::new("asset-9", 0.48, 0.52, 8, 1_045, true),
        preview_ok: true,
        submit: SubmitFixture::accepted("order-1", 1_110),
        verification: Some(VerificationFixture::verified(1_145)),
    };

    let outcome = DeterministicReplayHarness::default().run(&fixture);

    assert_eq!(outcome.reject_reason(), None);
    assert_eq!(outcome.order_side(), Some(OrderSide::Buy));
    assert_eq!(outcome.lifecycle_status_label(), Some("verified"));
    assert_eq!(
        outcome.trace().stage_started_at(Stage::ActivityObserved),
        Some(1_000)
    );
    assert_eq!(
        outcome.trace().stage_started_at(Stage::PositionsReconciled),
        Some(1_035)
    );
    assert_eq!(
        outcome.trace().stage_started_at(Stage::MarketQuoted),
        Some(1_045)
    );
    assert_eq!(
        outcome.trace().stage_started_at(Stage::OrderSubmitted),
        Some(1_110)
    );
    assert_eq!(
        outcome
            .trace()
            .stage_started_at(Stage::VerificationObserved),
        Some(1_145)
    );
}

#[test]
fn replay_harness_rejects_before_submit_when_positions_show_no_net_change() {
    let fixture = ReplayFixture {
        activity: ActivityEvent::new("leader-1", "0xtx", "BUY", "asset-9", 4, 1_000),
        positions: PositionFixtures::new(
            ("leader-1", "asset-9", 10, 1_000, 4),
            ("leader-1", "asset-9", 10, 1_035, 6),
        ),
        market: MarketFixture::new("asset-9", 0.48, 0.52, 8, 1_045, true),
        preview_ok: true,
        submit: SubmitFixture::accepted("order-1", 1_110),
        verification: None,
    };

    let outcome = DeterministicReplayHarness::default().run(&fixture);

    assert_eq!(outcome.reject_reason(), Some("no_net_position_change"));
    assert_eq!(outcome.lifecycle_status_label(), None);
    assert_eq!(outcome.trace().last_stage(), Some(Stage::ActivityObserved));
}

#[test]
fn replay_harness_fails_closed_when_remaining_budget_is_below_submit_requirement() {
    let fixture = ReplayFixture {
        activity: ActivityEvent::new("leader-1", "0xtx", "BUY", "asset-9", 4, 1_000),
        positions: PositionFixtures::new(
            ("leader-1", "asset-9", 10, 1_000, 4),
            ("leader-1", "asset-9", 14, 1_150, 6),
        ),
        market: MarketFixture::new("asset-9", 0.48, 0.52, 8, 1_170, true),
        preview_ok: true,
        submit: SubmitFixture::accepted("order-1", 1_205),
        verification: Some(VerificationFixture::verified(1_235)),
    };

    let outcome = DeterministicReplayHarness::default().run(&fixture);

    assert_eq!(outcome.reject_reason(), Some("latency_budget_exhausted"));
    assert_eq!(outcome.lifecycle_status_label(), None);
    assert_eq!(
        outcome.trace().stage_started_at(Stage::OrderSubmitted),
        None
    );
}

#[test]
fn replay_harness_records_submit_failure_without_entering_verification_pending() {
    let fixture = ReplayFixture {
        activity: ActivityEvent::new("leader-1", "0xtx", "SELL", "asset-9", 3, 1_000),
        positions: PositionFixtures::new(
            ("leader-1", "asset-9", 10, 1_000, 4),
            ("leader-1", "asset-9", 7, 1_035, 6),
        ),
        market: MarketFixture::new("asset-9", 0.48, 0.52, 8, 1_045, true),
        preview_ok: true,
        submit: SubmitFixture::rejected(),
        verification: None,
    };

    let outcome = DeterministicReplayHarness::default().run(&fixture);

    assert_eq!(outcome.reject_reason(), None);
    assert_eq!(outcome.order_side(), Some(OrderSide::Sell));
    assert_eq!(outcome.lifecycle_status_label(), Some("submit_failed"));
    assert!(outcome.lifecycle().unwrap().is_terminal());
    assert!(!outcome.lifecycle().unwrap().is_verification_pending());
}
