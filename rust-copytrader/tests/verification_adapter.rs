use rust_copytrader::adapters::verification::{
    VerificationAdapter, VerificationChannelEvent, VerificationChannelKind, VerificationCheckResult,
};
use rust_copytrader::execution::state_machine::OrderLifecycle;

#[test]
fn matching_verification_event_marks_lifecycle_verified() {
    let lifecycle = OrderLifecycle::new("corr-verify").mark_submitted(1_000);
    let adapter = VerificationAdapter::new(50);
    let event =
        VerificationChannelEvent::new("corr-verify", VerificationChannelKind::OrderMatched, 1_025);

    let result = adapter.evaluate(lifecycle, Some(event), 1_025);

    match result {
        VerificationCheckResult::Applied(lifecycle) => {
            assert_eq!(lifecycle.status_label(), "verified");
            assert!(lifecycle.is_terminal());
        }
        other => panic!("expected applied verified lifecycle, got {other:?}"),
    }
}

#[test]
fn mismatched_correlation_is_rejected_explicitly() {
    let lifecycle = OrderLifecycle::new("corr-verify").mark_submitted(1_000);
    let adapter = VerificationAdapter::new(50);
    let event =
        VerificationChannelEvent::new("corr-other", VerificationChannelKind::OrderMatched, 1_020);

    let result = adapter.evaluate(lifecycle, Some(event), 1_020);

    assert_eq!(
        result,
        VerificationCheckResult::Rejected("verification_correlation_mismatch".into())
    );
}

#[test]
fn missing_event_times_out_after_threshold() {
    let lifecycle = OrderLifecycle::new("corr-verify").mark_submitted(1_000);
    let adapter = VerificationAdapter::new(50);

    let result = adapter.evaluate(lifecycle, None, 1_051);

    match result {
        VerificationCheckResult::Applied(lifecycle) => {
            assert_eq!(lifecycle.status_label(), "verification_timeout");
            assert!(lifecycle.is_terminal());
        }
        other => panic!("expected timeout lifecycle, got {other:?}"),
    }
}

#[test]
fn pending_lifecycle_stays_pending_before_timeout_without_event() {
    let lifecycle = OrderLifecycle::new("corr-verify").mark_submitted(1_000);
    let adapter = VerificationAdapter::new(50);

    let result = adapter.evaluate(lifecycle, None, 1_030);

    assert_eq!(result, VerificationCheckResult::Pending);
}
