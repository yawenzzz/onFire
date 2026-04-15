#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageBudget {
    name: String,
    target_ms: u64,
}

impl StageBudget {
    pub fn new(name: impl Into<String>, target_ms: u64) -> Self {
        Self {
            name: name.into(),
            target_ms,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn target_ms(&self) -> u64 {
        self.target_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyBudget {
    hard_limit_ms: u64,
}

impl LatencyBudget {
    pub const fn new(hard_limit_ms: u64) -> Self {
        Self { hard_limit_ms }
    }

    pub const fn remaining_ms(&self, elapsed_ms: u64) -> Option<u64> {
        if elapsed_ms > self.hard_limit_ms {
            None
        } else {
            Some(self.hard_limit_ms - elapsed_ms)
        }
    }

    pub const fn can_schedule(&self, elapsed_ms: u64, stage: &StageBudget) -> bool {
        match self.remaining_ms(elapsed_ms) {
            Some(remaining_ms) => remaining_ms >= stage.target_ms(),
            None => false,
        }
    }

    pub const fn hard_limit_ms(&self) -> u64 {
        self.hard_limit_ms
    }
}
