use crate::domain::position_targeting::{
    AssetId, ConditionId, EventId, LeaderConfig, LeaderPosition, LeaderState, LeaderValue,
    PricePpm, ProvisionalDelta, UnixMs, UsdcMicros,
};
use crate::wallet_filter::{
    extract_json_bool_field, extract_json_field, json_objects, parse_f64, parse_iso8601_timestamp,
};

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

pub fn parse_leader_positions_payload(content: &str) -> Vec<LeaderPosition> {
    json_objects(content)
        .into_iter()
        .map(|object| LeaderPosition {
            asset: AssetId(extract_json_field(&object, "asset").unwrap_or_default()),
            condition: ConditionId(extract_json_field(&object, "conditionId").unwrap_or_default()),
            event: extract_json_field(&object, "eventId").map(EventId),
            outcome: extract_json_field(&object, "outcome").unwrap_or_else(|| "YES".to_string()),
            size: parse_decimal_micros(
                extract_json_field(&object, "size")
                    .as_deref()
                    .unwrap_or("0"),
            ),
            avg_price_ppm: parse_price_ppm(
                extract_json_field(&object, "avgPrice")
                    .as_deref()
                    .unwrap_or("0"),
            ),
            initial_value: parse_decimal_micros(
                extract_json_field(&object, "initialValue")
                    .as_deref()
                    .unwrap_or("0"),
            ),
            current_value: parse_decimal_micros(
                extract_json_field(&object, "currentValue")
                    .as_deref()
                    .unwrap_or("0"),
            ),
            end_ts_ms: parse_end_ts_ms(
                extract_json_field(&object, "endDate")
                    .as_deref()
                    .unwrap_or("1970-01-01"),
            ),
            neg_risk: extract_json_bool_field(&object, "negativeRisk").unwrap_or(false),
            slug: extract_json_field(&object, "slug").unwrap_or_default(),
        })
        .collect()
}

pub fn parse_total_value_payload(content: &str) -> Option<UsdcMicros> {
    let object = json_objects(content).into_iter().next()?;
    Some(parse_decimal_micros(
        extract_json_field(&object, "value")
            .as_deref()
            .unwrap_or("0"),
    ))
}

pub fn update_leader_value_ewma(
    previous: Option<&LeaderValue>,
    spot_value: UsdcMicros,
    observed_at_ms: UnixMs,
    beta_bps: u32,
) -> LeaderValue {
    let beta = beta_bps.min(10_000) as i128;
    let ewma_value = if let Some(previous) = previous {
        (((spot_value as i128) * beta) + ((previous.ewma_value as i128) * (10_000 - beta))) / 10_000
    } else {
        spot_value as i128
    } as UsdcMicros;

    LeaderValue {
        spot_value,
        ewma_value,
        last_update_ms: observed_at_ms,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_leader_state(
    config: LeaderConfig,
    spot_value: UsdcMicros,
    previous_value: Option<&LeaderValue>,
    positions: Vec<LeaderPosition>,
    provisional_deltas: Vec<ProvisionalDelta>,
    observed_at_ms: UnixMs,
    lag_ms: UnixMs,
    stale_ms: UnixMs,
    avg_corr_bps: u16,
    ewma_beta_bps: u32,
) -> LeaderState {
    LeaderState {
        config,
        value: update_leader_value_ewma(previous_value, spot_value, observed_at_ms, ewma_beta_bps),
        positions,
        provisional_deltas,
        lag_ms,
        stale_ms,
        avg_corr_bps,
    }
}

fn parse_decimal_micros(value: &str) -> i64 {
    let value = value.trim();
    if value.is_empty() {
        return 0;
    }
    let negative = value.starts_with('-');
    let value = value.trim_start_matches('-');
    let (whole, frac) = value.split_once('.').unwrap_or((value, ""));
    let whole = whole.parse::<i64>().unwrap_or(0);
    let mut frac_buf = frac.to_string();
    while frac_buf.len() < 6 {
        frac_buf.push('0');
    }
    let frac_value = frac_buf[..6.min(frac_buf.len())]
        .parse::<i64>()
        .unwrap_or(0);
    let scaled = whole.saturating_mul(1_000_000).saturating_add(frac_value);
    if negative { -scaled } else { scaled }
}

fn parse_price_ppm(value: &str) -> PricePpm {
    (parse_f64(value).unwrap_or(0.0) * 1_000_000.0).round() as PricePpm
}

fn parse_end_ts_ms(value: &str) -> UnixMs {
    if let Some(ts) = parse_iso8601_timestamp(value) {
        return (ts as UnixMs) * 1000;
    }
    let value = value.trim();
    let mut parts = value.split('-');
    let year = parts.next().and_then(|part| part.parse::<i64>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    match (year, month, day) {
        (Some(year), Some(month), Some(day)) => {
            let month = month.clamp(1, 12);
            let day = day.clamp(1, 31);
            let (year, month) = if month <= 2 {
                (year - 1, month as i64 + 12)
            } else {
                (year, month as i64)
            };
            let era = year.div_euclid(400);
            let yoe = year - era * 400;
            let doy = (153 * (month - 3) + 2) / 5 + day as i64 - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            let days = era * 146097 + doe - 719468;
            days.saturating_mul(86_400_000)
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::position_targeting::{LeaderId, ProvisionalDelta};

    #[test]
    fn parse_leader_positions_payload_maps_polymarket_shape() {
        let positions = parse_leader_positions_payload(
            r#"[{"proxyWallet":"0xabc","asset":"asset-1","conditionId":"0xcond","size":1625004.0894,"avgPrice":0.4382,"initialValue":712104.417,"currentValue":0,"slug":"market-a","eventId":"196247","outcome":"Yes","endDate":"2026-02-15","negativeRisk":true}]"#,
        );

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].asset.0, "asset-1");
        assert_eq!(positions[0].condition.0, "0xcond");
        assert_eq!(
            positions[0].event.as_ref().map(|value| value.0.as_str()),
            Some("196247")
        );
        assert_eq!(positions[0].outcome, "Yes");
        assert_eq!(positions[0].size, 1_625_004_089_400);
        assert_eq!(positions[0].avg_price_ppm, 438_200);
        assert_eq!(positions[0].initial_value, 712_104_417_000);
        assert!(positions[0].neg_risk);
    }

    #[test]
    fn parse_total_value_payload_reads_value() {
        assert_eq!(
            parse_total_value_payload(r#"[{"user":"0xabc","value":123.456789}]"#),
            Some(123_456_789)
        );
    }

    #[test]
    fn update_leader_value_ewma_blends_previous_and_spot() {
        let previous = LeaderValue {
            spot_value: 100,
            ewma_value: 200,
            last_update_ms: 1,
        };
        let updated = update_leader_value_ewma(Some(&previous), 1_000, 5, 2_500);
        assert_eq!(updated.spot_value, 1_000);
        assert_eq!(updated.ewma_value, 400);
        assert_eq!(updated.last_update_ms, 5);
    }

    #[test]
    fn build_leader_state_wraps_value_positions_and_deltas() {
        let config = LeaderConfig {
            leader: LeaderId("leader-1".into()),
            base_score_bps: 9_000,
            alpha_bps: 3_000,
            enabled: true,
        };
        let positions = parse_leader_positions_payload(
            r#"[{"asset":"asset-1","conditionId":"0xcond","size":1,"avgPrice":0.4,"initialValue":10,"currentValue":11,"slug":"market-a","eventId":"196247","outcome":"Yes","endDate":"2026-02-15","negativeRisk":false}]"#,
        );
        let delta = ProvisionalDelta {
            leader: LeaderId("leader-1".into()),
            asset: AssetId("asset-1".into()),
            signed_risk_usdc: 5,
            leader_event_ts_ms: 1,
            local_recv_ts_ms: 2,
            expires_at_ms: 3,
            tx_hash: "0xtx".into(),
        };
        let state = build_leader_state(
            config,
            1_000,
            None,
            positions,
            vec![delta],
            10,
            11,
            12,
            1_000,
            5_000,
        );

        assert_eq!(state.value.spot_value, 1_000);
        assert_eq!(state.value.ewma_value, 1_000);
        assert_eq!(state.positions.len(), 1);
        assert_eq!(state.provisional_deltas.len(), 1);
        assert_eq!(state.lag_ms, 11);
        assert_eq!(state.stale_ms, 12);
    }
}
