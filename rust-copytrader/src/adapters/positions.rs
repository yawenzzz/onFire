#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionSnapshot {
    pub proxy_wallet: String,
    pub asset_id: String,
    pub current_size: i64,
    pub observed_at_ms: u64,
    pub snapshot_age_ms: u64,
}

impl PositionSnapshot {
    pub fn new(
        proxy_wallet: impl Into<String>,
        asset_id: impl Into<String>,
        current_size: i64,
        observed_at_ms: u64,
        snapshot_age_ms: u64,
    ) -> Self {
        Self {
            proxy_wallet: proxy_wallet.into(),
            asset_id: asset_id.into(),
            current_size,
            observed_at_ms,
            snapshot_age_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderPositionDelta {
    pub proxy_wallet: String,
    pub asset_id: String,
    pub previous_size: i64,
    pub current_size: i64,
    pub delta_size: i64,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PositionsOutcome {
    Rejected(String),
    NoNetChange,
    Delta(LeaderPositionDelta),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionsReconciler {
    max_snapshot_age_ms: u64,
}

impl PositionsReconciler {
    pub const fn new(max_snapshot_age_ms: u64) -> Self {
        Self {
            max_snapshot_age_ms,
        }
    }

    pub fn reconcile(
        &self,
        previous: &PositionSnapshot,
        current: &PositionSnapshot,
    ) -> PositionsOutcome {
        if current.snapshot_age_ms > self.max_snapshot_age_ms {
            return PositionsOutcome::Rejected("positions_snapshot_stale".to_string());
        }

        if previous.proxy_wallet != current.proxy_wallet || previous.asset_id != current.asset_id {
            return PositionsOutcome::Rejected("positions_subject_mismatch".to_string());
        }

        let delta_size = current.current_size - previous.current_size;
        if delta_size == 0 {
            return PositionsOutcome::NoNetChange;
        }

        PositionsOutcome::Delta(LeaderPositionDelta {
            proxy_wallet: current.proxy_wallet.clone(),
            asset_id: current.asset_id.clone(),
            previous_size: previous.current_size,
            current_size: current.current_size,
            delta_size,
            observed_at_ms: current.observed_at_ms,
        })
    }
}
