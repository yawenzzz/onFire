use crate::config::{ActivityMode, LiveModeGate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapDecision {
    LiveListen,
    ShadowPoll,
    Replay,
    Blocked(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeBootstrap {
    requested_mode: ActivityMode,
    gate: LiveModeGate,
}

impl RuntimeBootstrap {
    pub const fn new(requested_mode: ActivityMode, gate: LiveModeGate) -> Self {
        Self {
            requested_mode,
            gate,
        }
    }

    pub fn decide(&self) -> BootstrapDecision {
        match self.requested_mode {
            ActivityMode::LiveListen => self
                .gate
                .blocked_reason()
                .map(BootstrapDecision::Blocked)
                .unwrap_or(BootstrapDecision::LiveListen),
            ActivityMode::ShadowPoll => BootstrapDecision::ShadowPoll,
            ActivityMode::Replay => BootstrapDecision::Replay,
        }
    }
}
