use crate::execution::state_machine::{OrderLifecycle, VerificationOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationChannelKind {
    OrderMatched,
    OrderMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationChannelEvent {
    pub correlation_id: String,
    pub kind: VerificationChannelKind,
    pub observed_at_ms: u64,
}

impl VerificationChannelEvent {
    pub fn new(
        correlation_id: impl Into<String>,
        kind: VerificationChannelKind,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            kind,
            observed_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationCheckResult {
    Pending,
    Applied(OrderLifecycle),
    Rejected(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationAdapter {
    timeout_ms: u64,
}

impl VerificationAdapter {
    pub const fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    pub fn evaluate(
        &self,
        lifecycle: OrderLifecycle,
        event: Option<VerificationChannelEvent>,
        now_ms: u64,
    ) -> VerificationCheckResult {
        if !lifecycle.is_verification_pending() {
            return VerificationCheckResult::Applied(lifecycle);
        }

        if let Some(event) = event {
            if event.correlation_id != lifecycle.correlation_id() {
                return VerificationCheckResult::Rejected(
                    "verification_correlation_mismatch".to_string(),
                );
            }

            let outcome = match event.kind {
                VerificationChannelKind::OrderMatched => VerificationOutcome::Verified {
                    verified_at_ms: event.observed_at_ms,
                },
                VerificationChannelKind::OrderMismatch => VerificationOutcome::Mismatch {
                    observed_at_ms: event.observed_at_ms,
                },
            };
            return VerificationCheckResult::Applied(lifecycle.apply_verification(outcome));
        }

        match lifecycle.submitted_at_ms() {
            Some(submitted_at_ms) if now_ms.saturating_sub(submitted_at_ms) >= self.timeout_ms => {
                VerificationCheckResult::Applied(lifecycle.apply_verification(
                    VerificationOutcome::Timeout {
                        observed_at_ms: now_ms,
                    },
                ))
            }
            Some(_) => VerificationCheckResult::Pending,
            None => VerificationCheckResult::Rejected(
                "verification_missing_submit_timestamp".to_string(),
            ),
        }
    }
}
