use rust_copytrader::pipeline::trace_context::Stage;
use rust_copytrader::replay::fixture::{
    ReplayFixture, ReplayPreviewResult, ReplaySubmitResult, ReplayVerificationFrame,
};
use rust_copytrader::replay::harness::{
    ReplayHarness, ReplayRejectReason, ReplayRunOutcome, ReplayStage,
};

#[test]
fn replay_harness_preserves_fixed_stage_order_for_verified_submit() {
    let fixture = ReplayFixture::success_buy_follow();
    let outcome = ReplayHarness::default().run(&fixture);

    match outcome {
        ReplayRunOutcome::Accepted(accepted) => {
            assert_eq!(accepted.submit_elapsed_ms, 60);
            assert_eq!(accepted.lifecycle.status_label(), "verified");
            assert_eq!(
                accepted.stage_order,
                vec![
                    ReplayStage::ActivityObserved,
                    ReplayStage::PositionsReconciled,
                    ReplayStage::MarketQuoted,
                    ReplayStage::OrderSubmitted,
                    ReplayStage::VerificationObserved,
                ]
            );
            assert_eq!(
                accepted.trace.stage_started_at(Stage::VerificationObserved),
                Some(1_082)
            );
        }
        other => panic!("expected accepted replay, got {other:?}"),
    }
}

#[test]
fn replay_harness_rejects_when_positions_show_no_net_change() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.current_position.current_size = fixture.previous_position.current_size;

    let outcome = ReplayHarness::default().run(&fixture);

    match outcome {
        ReplayRunOutcome::Rejected(rejected) => {
            assert_eq!(
                rejected.reason,
                ReplayRejectReason::Positions("positions_no_net_change".into())
            );
            assert_eq!(rejected.stage_order, vec![ReplayStage::ActivityObserved]);
        }
        other => panic!("expected positions rejection, got {other:?}"),
    }
}

#[test]
fn replay_harness_keeps_submit_failures_out_of_verification_pending_state() {
    let mut fixture = ReplayFixture::success_buy_follow();
    fixture.preview = ReplayPreviewResult::Accepted;
    fixture.submit = ReplaySubmitResult::Rejected;
    fixture.verification = ReplayVerificationFrame::Timeout {
        observed_at_ms: 1_140,
    };

    let outcome = ReplayHarness::default().run(&fixture);

    match outcome {
        ReplayRunOutcome::Rejected(rejected) => {
            assert_eq!(rejected.reason, ReplayRejectReason::SubmitRejected);
            let lifecycle = rejected
                .lifecycle
                .expect("submit rejection keeps lifecycle");
            assert_eq!(lifecycle.status_label(), "submit_failed");
            assert!(!lifecycle.is_verification_pending());
        }
        other => panic!("expected submit rejection, got {other:?}"),
    }
}
