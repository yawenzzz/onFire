use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMetrics {
    submitted: u64,
    verification_timeouts: u64,
    rejected_total: u64,
    reject_counts: BTreeMap<String, u64>,
}

impl RuntimeMetrics {
    pub fn record_submit(&mut self) {
        self.submitted += 1;
    }

    pub fn record_reject(&mut self, reason: &str) {
        self.rejected_total += 1;
        *self.reject_counts.entry(reason.to_string()).or_default() += 1;
    }

    pub fn record_verification_timeout(&mut self) {
        self.verification_timeouts += 1;
    }

    pub const fn submitted(&self) -> u64 {
        self.submitted
    }

    pub const fn rejected_total(&self) -> u64 {
        self.rejected_total
    }

    pub const fn verification_timeouts(&self) -> u64 {
        self.verification_timeouts
    }

    pub fn reject_count(&self, reason: &str) -> u64 {
        self.reject_counts.get(reason).copied().unwrap_or(0)
    }
}
