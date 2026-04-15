use rust_copytrader::pipeline::orchestrator::HotPathOrchestrator;
use rust_copytrader::replay::fixture::{
    ReplayFixture, ReplaySubmitResult, ReplayVerificationFrame,
};

#[test]
fn orchestrator_accepts_verified_happy_path() {
    let fixture = ReplayFixture::success_buy_follow();
    let outcome = HotPathOrchestrator::default().run(&fixture);

    assert_eq!(outcome.reject_reason(), None);
    assert_eq!(outcome.lifecycle_status_label(), Some("verified"));
    assert_eq!(
        outcome.order_side(),
        Some(rust_copytrader::adapters::order_api::OrderSide::Buy)
    );
}

#[test]
fn orchestrator_keeps_submit_rejection_out_of_verification_pending() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.submit = ReplaySubmitResult::Rejected;
    fixture.verification = ReplayVerificationFrame::Timeout {
        observed_at_ms: 1_140,
    };

    let outcome = HotPathOrchestrator::default().run(&fixture);
    let lifecycle = outcome.lifecycle().expect("lifecycle expected");
    assert_eq!(lifecycle.status_label(), "submit_failed");
    assert!(!lifecycle.is_verification_pending());
}

#[test]
fn orchestrator_rejects_no_net_position_change() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.current_position.current_size = fixture.previous_position.current_size;

    let outcome = HotPathOrchestrator::default().run(&fixture);
    assert_eq!(outcome.reject_reason(), Some("no_net_position_change"));
}

#[test]
fn orchestrator_rejects_over_budget_before_submit() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.quote.observed_at_ms = 1_190;
    fixture.submit_ack_at_ms = 1_230;

    let outcome = HotPathOrchestrator::default().run(&fixture);
    assert_eq!(outcome.reject_reason(), Some("latency_budget_exhausted"));
}

#[test]
fn orchestrator_rejects_closed_market_before_submit() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.quote.market_open = false;

    let outcome = HotPathOrchestrator::default().run(&fixture);
    assert_eq!(outcome.reject_reason(), Some("market_not_open"));
}
