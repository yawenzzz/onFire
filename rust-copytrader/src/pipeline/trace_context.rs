use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    ActivityObserved,
    PositionsReconciled,
    OrderSubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    leader_id: String,
    correlation_id: String,
    started_at_ms: u64,
    last_stage: Option<Stage>,
    stage_times: BTreeMap<Stage, u64>,
}

impl TraceContext {
    pub fn new(
        leader_id: impl Into<String>,
        correlation_id: impl Into<String>,
        started_at_ms: u64,
    ) -> Self {
        Self {
            leader_id: leader_id.into(),
            correlation_id: correlation_id.into(),
            started_at_ms,
            last_stage: None,
            stage_times: BTreeMap::new(),
        }
    }

    pub fn mark(&mut self, stage: Stage, observed_at_ms: u64) {
        self.stage_times.insert(stage, observed_at_ms);
        self.last_stage = Some(stage);
    }

    pub fn stage_started_at(&self, stage: Stage) -> Option<u64> {
        self.stage_times.get(&stage).copied()
    }

    pub const fn last_stage(&self) -> Option<Stage> {
        self.last_stage
    }

    pub fn total_elapsed_ms(&self) -> u64 {
        self.last_stage
            .and_then(|stage| self.stage_started_at(stage))
            .map(|stage_started_at| stage_started_at.saturating_sub(self.started_at_ms))
            .unwrap_or(0)
    }

    pub fn leader_id(&self) -> &str {
        &self.leader_id
    }

    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}
