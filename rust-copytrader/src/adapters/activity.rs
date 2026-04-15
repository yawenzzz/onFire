use crate::config::{ActivityMode, LiveModeGate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityAdapterMode {
    LiveListen,
    ShadowPoll,
    Replay,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct ActivitySourceSelector {
    requested_mode: ActivityMode,
    gate: LiveModeGate,
}

impl ActivitySourceSelector {
    pub const fn new(requested_mode: ActivityMode, gate: LiveModeGate) -> Self {
        Self {
            requested_mode,
            gate,
        }
    }

    pub fn resolve_mode(&self) -> ActivityAdapterMode {
        match self.requested_mode {
            ActivityMode::LiveListen if self.gate.unlocked() => ActivityAdapterMode::LiveListen,
            ActivityMode::LiveListen => ActivityAdapterMode::Blocked,
            ActivityMode::ShadowPoll => ActivityAdapterMode::ShadowPoll,
            ActivityMode::Replay => ActivityAdapterMode::Replay,
        }
    }

    pub fn blocked_reason(&self) -> Option<String> {
        match self.resolve_mode() {
            ActivityAdapterMode::Blocked => self.gate.blocked_reason(),
            _ => None,
        }
    }
}
