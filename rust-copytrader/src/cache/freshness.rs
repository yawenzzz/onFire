#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessGate {
    max_age_ms: u64,
}

impl FreshnessGate {
    pub const fn new(max_age_ms: u64) -> Self {
        Self { max_age_ms }
    }

    pub const fn is_fresh(&self, observed_age_ms: u64) -> bool {
        observed_age_ms <= self.max_age_ms
    }

    pub const fn max_age_ms(&self) -> u64 {
        self.max_age_ms
    }
}
