use rust_copytrader::replay::fixture::ReplayFixture;
use rust_copytrader::replay::harness::{
    ReplayHarness, ReplayRejectReason, ReplayRunOutcome, ReplayStage,
};

#[test]
fn replay_harness_rejects_before_submit_when_remaining_budget_is_exhausted() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.quote.observed_at_ms = 1_150;
    fixture.submit_ack_at_ms = 1_205;

    let outcome = ReplayHarness::default().run(&fixture);

    match outcome {
        ReplayRunOutcome::Rejected(rejected) => {
            assert_eq!(
                rejected.reason,
                ReplayRejectReason::BudgetExceeded {
                    stage: ReplayStage::OrderSubmitted,
                    elapsed_ms: 150,
                }
            );
            assert_eq!(rejected.reason.code(), "submit_over_budget");
            assert_eq!(
                rejected.stage_order,
                vec![
                    ReplayStage::ActivityObserved,
                    ReplayStage::PositionsReconciled,
                    ReplayStage::MarketQuoted,
                ]
            );
            assert!(rejected.lifecycle.is_none());
        }
        other => panic!("expected over-budget rejection, got {other:?}"),
    }
}

#[test]
fn replay_harness_accepts_within_budget_success_path() {
    let fixture = ReplayFixture::success_buy_follow();
    let outcome = ReplayHarness::default().run(&fixture);

    match outcome {
        ReplayRunOutcome::Accepted(accepted) => {
            assert_eq!(accepted.submit_elapsed_ms, 60);
            assert!(accepted.submit_elapsed_ms <= 200);
        }
        other => panic!("expected accepted replay, got {other:?}"),
    }
}
