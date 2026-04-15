use rust_copytrader::execution::state_machine::{
    OrderLifecycle, SubmitFailure, VerificationOutcome,
};

#[test]
fn submit_failure_is_terminal_for_that_attempt() {
    let lifecycle =
        OrderLifecycle::new("corr-1").mark_submit_failed(SubmitFailure::PreviewRejected);

    assert!(lifecycle.is_terminal());
    assert!(!lifecycle.is_verification_pending());
    assert_eq!(lifecycle.status_label(), "submit_failed");
}

#[test]
fn accepted_submit_enters_verification_pending_until_outcome_arrives() {
    let pending = OrderLifecycle::new("corr-2").mark_submitted(1_250);
    assert!(pending.is_verification_pending());
    assert_eq!(pending.status_label(), "submitted_unverified");

    let verified = pending
        .clone()
        .apply_verification(VerificationOutcome::Verified {
            verified_at_ms: 1_280,
        });
    assert!(verified.is_terminal());
    assert_eq!(verified.status_label(), "verified");

    let mismatch = pending.apply_verification(VerificationOutcome::Mismatch {
        observed_at_ms: 1_310,
    });
    assert!(mismatch.is_terminal());
    assert_eq!(mismatch.status_label(), "verification_mismatch");
}
