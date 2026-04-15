use crate::pipeline::trace_context::{Stage, TraceContext};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LatencyReport {
    samples: u64,
    total_elapsed_max_ms: u64,
    latest_stage_deltas: BTreeMap<Stage, u64>,
}

impl LatencyReport {
    pub fn record_trace(&mut self, trace: &TraceContext) {
        self.samples += 1;
        self.total_elapsed_max_ms = self.total_elapsed_max_ms.max(trace.total_elapsed_ms());

        let ordered = [
            Stage::ActivityObserved,
            Stage::PositionsReconciled,
            Stage::MarketQuoted,
            Stage::PreTradeValidated,
            Stage::OrderSubmitted,
            Stage::VerificationObserved,
        ];

        let mut prev = trace.stage_started_at(Stage::ActivityObserved);
        for stage in ordered.into_iter().skip(1) {
            if let (Some(prev_ms), Some(current_ms)) = (prev, trace.stage_started_at(stage)) {
                self.latest_stage_deltas
                    .insert(stage, current_ms.saturating_sub(prev_ms));
                prev = Some(current_ms);
            }
        }
    }

    pub const fn samples(&self) -> u64 {
        self.samples
    }

    pub fn stage_delta_ms(&self, stage: Stage) -> Option<u64> {
        self.latest_stage_deltas.get(&stage).copied()
    }

    pub const fn total_elapsed_max_ms(&self) -> u64 {
        self.total_elapsed_max_ms
    }

    pub const fn latest_stage_deltas(&self) -> &BTreeMap<Stage, u64> {
        &self.latest_stage_deltas
    }
}
