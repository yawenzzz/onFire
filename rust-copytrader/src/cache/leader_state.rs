use std::collections::HashMap;

use crate::adapters::positions::PositionSnapshot;
use crate::domain::events::ActivityEvent;

use super::freshness::FreshnessGate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderMarketState {
    pub leader_id: String,
    pub asset_id: String,
    pub last_activity_at_ms: u64,
    pub last_transaction_hash: String,
    pub last_position_size: i64,
    pub position_observed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderStateError {
    MissingMarket {
        leader_id: String,
        asset_id: String,
    },
    MissingActivity {
        leader_id: String,
        asset_id: String,
    },
    MissingPosition {
        leader_id: String,
        asset_id: String,
    },
    StaleActivity {
        leader_id: String,
        asset_id: String,
        observed_age_ms: u64,
    },
    StalePosition {
        leader_id: String,
        asset_id: String,
        observed_age_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LeaderMarketKey {
    leader_id: String,
    asset_id: String,
}

impl LeaderMarketKey {
    fn new(leader_id: impl Into<String>, asset_id: impl Into<String>) -> Self {
        Self {
            leader_id: leader_id.into(),
            asset_id: asset_id.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct LeaderMarketEntry {
    last_activity: Option<ActivityEvent>,
    last_position: Option<PositionSnapshot>,
}

#[derive(Debug, Clone)]
pub struct LeaderStateCache {
    freshness: FreshnessGate,
    entries: HashMap<LeaderMarketKey, LeaderMarketEntry>,
}

impl LeaderStateCache {
    pub fn new(max_age_ms: u64) -> Self {
        Self {
            freshness: FreshnessGate::new(max_age_ms),
            entries: HashMap::new(),
        }
    }

    pub fn record_activity(&mut self, event: ActivityEvent) -> bool {
        let key = LeaderMarketKey::new(event.proxy_wallet.clone(), event.asset_id.clone());
        let entry = self.entries.entry(key).or_default();
        if matches!(
            entry.last_activity.as_ref(),
            Some(existing) if existing.observed_at_ms > event.observed_at_ms
        ) {
            return false;
        }

        entry.last_activity = Some(event);
        true
    }

    pub fn update_position(&mut self, snapshot: PositionSnapshot) -> bool {
        let key = LeaderMarketKey::new(snapshot.proxy_wallet.clone(), snapshot.asset_id.clone());
        let entry = self.entries.entry(key).or_default();
        if matches!(
            entry.last_position.as_ref(),
            Some(existing) if existing.observed_at_ms > snapshot.observed_at_ms
        ) {
            return false;
        }

        entry.last_position = Some(snapshot);
        true
    }

    pub fn market_state(
        &self,
        leader_id: impl AsRef<str>,
        asset_id: impl AsRef<str>,
        now_ms: u64,
    ) -> Result<LeaderMarketState, LeaderStateError> {
        let leader_id = leader_id.as_ref();
        let asset_id = asset_id.as_ref();
        let key = LeaderMarketKey::new(leader_id.to_string(), asset_id.to_string());
        let entry = self
            .entries
            .get(&key)
            .ok_or_else(|| LeaderStateError::MissingMarket {
                leader_id: leader_id.to_string(),
                asset_id: asset_id.to_string(),
            })?;
        let activity =
            entry
                .last_activity
                .as_ref()
                .ok_or_else(|| LeaderStateError::MissingActivity {
                    leader_id: leader_id.to_string(),
                    asset_id: asset_id.to_string(),
                })?;
        let activity_age_ms = now_ms.saturating_sub(activity.observed_at_ms);
        if !self.freshness.is_fresh(activity_age_ms) {
            return Err(LeaderStateError::StaleActivity {
                leader_id: leader_id.to_string(),
                asset_id: asset_id.to_string(),
                observed_age_ms: activity_age_ms,
            });
        }

        let position =
            entry
                .last_position
                .as_ref()
                .ok_or_else(|| LeaderStateError::MissingPosition {
                    leader_id: leader_id.to_string(),
                    asset_id: asset_id.to_string(),
                })?;
        let position_age_ms = position
            .snapshot_age_ms
            .saturating_add(now_ms.saturating_sub(position.observed_at_ms));
        if !self.freshness.is_fresh(position_age_ms) {
            return Err(LeaderStateError::StalePosition {
                leader_id: leader_id.to_string(),
                asset_id: asset_id.to_string(),
                observed_age_ms: position_age_ms,
            });
        }

        Ok(LeaderMarketState {
            leader_id: leader_id.to_string(),
            asset_id: asset_id.to_string(),
            last_activity_at_ms: activity.observed_at_ms,
            last_transaction_hash: activity.transaction_hash.clone(),
            last_position_size: position.current_size,
            position_observed_at_ms: position.observed_at_ms,
        })
    }
}
