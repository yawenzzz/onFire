use crate::pipeline::orchestrator::{HotPathOrchestrator, PipelineOutcome};
use crate::replay::fixture::ReplayFixture;

#[derive(Debug, Clone, Default)]
pub struct DeterministicReplayHarness {
    orchestrator: HotPathOrchestrator,
}

impl DeterministicReplayHarness {
    pub fn run(&self, fixture: &ReplayFixture) -> PipelineOutcome {
        self.orchestrator.run(fixture)
    }
}
