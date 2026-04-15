#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityMode {
    LiveListen,
    ShadowPoll,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportAdapterKind {
    LiveListen,
    ShadowPoll,
    Replay,
}

impl TransportAdapterKind {
    pub const fn activity_mode(self) -> ActivityMode {
        match self {
            Self::LiveListen => ActivityMode::LiveListen,
            Self::ShadowPoll => ActivityMode::ShadowPoll,
            Self::Replay => ActivityMode::Replay,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportBoundaryConfig {
    pub activity: TransportAdapterKind,
    pub positions: TransportAdapterKind,
    pub market: TransportAdapterKind,
    pub verification: TransportAdapterKind,
}

impl TransportBoundaryConfig {
    pub const fn new(
        activity: TransportAdapterKind,
        positions: TransportAdapterKind,
        market: TransportAdapterKind,
        verification: TransportAdapterKind,
    ) -> Self {
        Self {
            activity,
            positions,
            market,
            verification,
        }
    }

    pub const fn for_mode(mode: ActivityMode) -> Self {
        let kind = match mode {
            ActivityMode::LiveListen => TransportAdapterKind::LiveListen,
            ActivityMode::ShadowPoll => TransportAdapterKind::ShadowPoll,
            ActivityMode::Replay => TransportAdapterKind::Replay,
        };

        Self::new(kind, kind, kind, kind)
    }

    pub fn requested_mode(&self) -> Result<ActivityMode, String> {
        if self.positions != self.activity {
            return Err("positions_transport_mode_mismatch".to_string());
        }
        if self.market != self.activity {
            return Err("market_transport_mode_mismatch".to_string());
        }
        if self.verification != self.activity {
            return Err("verification_transport_mode_mismatch".to_string());
        }

        Ok(self.activity.activity_mode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveModeGate {
    mode: ActivityMode,
    pub activity_source_verified: bool,
    pub activity_source_under_budget: bool,
    pub activity_capability_detected: bool,
    pub positions_under_budget: bool,
    pub execution_surface_ready: bool,
}

impl LiveModeGate {
    pub const fn for_mode(mode: ActivityMode) -> Self {
        Self {
            mode,
            activity_source_verified: false,
            activity_source_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            execution_surface_ready: false,
        }
    }

    pub fn blocked_reason(&self) -> Option<String> {
        match self.mode {
            ActivityMode::LiveListen => {
                if !self.activity_source_verified {
                    Some("activity_source_unverified".to_string())
                } else if !self.activity_source_under_budget {
                    Some("activity_source_over_budget".to_string())
                } else if !self.activity_capability_detected {
                    Some("activity_capability_missing".to_string())
                } else if !self.positions_under_budget {
                    Some("positions_over_budget".to_string())
                } else if !self.execution_surface_ready {
                    Some("execution_surface_not_ready".to_string())
                } else {
                    None
                }
            }
            ActivityMode::ShadowPoll | ActivityMode::Replay => None,
        }
    }

    pub fn unlocked(&self) -> bool {
        self.blocked_reason().is_none()
    }
}
