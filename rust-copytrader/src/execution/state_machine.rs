#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitFailure {
    PreviewRejected,
    SubmitRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    Verified { verified_at_ms: u64 },
    Mismatch { observed_at_ms: u64 },
    Timeout { observed_at_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OrderStatus {
    Created,
    SubmitFailed(SubmitFailure),
    SubmittedUnverified { submitted_at_ms: u64 },
    Verified { verified_at_ms: u64 },
    VerificationMismatch { observed_at_ms: u64 },
    VerificationTimeout { observed_at_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecycle {
    correlation_id: String,
    status: OrderStatus,
}

impl OrderLifecycle {
    pub fn new(correlation_id: impl Into<String>) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            status: OrderStatus::Created,
        }
    }

    pub fn mark_submit_failed(mut self, failure: SubmitFailure) -> Self {
        self.status = OrderStatus::SubmitFailed(failure);
        self
    }

    pub fn mark_submitted(mut self, submitted_at_ms: u64) -> Self {
        self.status = OrderStatus::SubmittedUnverified { submitted_at_ms };
        self
    }

    pub fn apply_verification(mut self, outcome: VerificationOutcome) -> Self {
        self.status = match outcome {
            VerificationOutcome::Verified { verified_at_ms } => {
                OrderStatus::Verified { verified_at_ms }
            }
            VerificationOutcome::Mismatch { observed_at_ms } => {
                OrderStatus::VerificationMismatch { observed_at_ms }
            }
            VerificationOutcome::Timeout { observed_at_ms } => {
                OrderStatus::VerificationTimeout { observed_at_ms }
            }
        };
        self
    }

    pub fn is_verification_pending(&self) -> bool {
        matches!(self.status, OrderStatus::SubmittedUnverified { .. })
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::SubmitFailed(_)
                | OrderStatus::Verified { .. }
                | OrderStatus::VerificationMismatch { .. }
                | OrderStatus::VerificationTimeout { .. }
        )
    }

    pub fn status_label(&self) -> &'static str {
        match self.status {
            OrderStatus::Created => "created",
            OrderStatus::SubmitFailed(_) => "submit_failed",
            OrderStatus::SubmittedUnverified { .. } => "submitted_unverified",
            OrderStatus::Verified { .. } => "verified",
            OrderStatus::VerificationMismatch { .. } => "verification_mismatch",
            OrderStatus::VerificationTimeout { .. } => "verification_timeout",
        }
    }

    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}
