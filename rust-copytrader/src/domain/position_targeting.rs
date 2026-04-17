use std::collections::{BTreeMap, HashMap};

pub type UsdcMicros = i64;
pub type SharesMicros = i64;
pub type PricePpm = i32;
pub type UnixMs = i64;

const DEFAULT_EXPOSURE_EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeaderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConditionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderConfig {
    pub leader: LeaderId,
    pub base_score_bps: u32,
    pub alpha_bps: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderValue {
    pub spot_value: UsdcMicros,
    pub ewma_value: UsdcMicros,
    pub last_update_ms: UnixMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderPosition {
    pub asset: AssetId,
    pub condition: ConditionId,
    pub event: Option<EventId>,
    pub outcome: String,
    pub size: SharesMicros,
    pub avg_price_ppm: PricePpm,
    pub initial_value: UsdcMicros,
    pub current_value: UsdcMicros,
    pub end_ts_ms: UnixMs,
    pub neg_risk: bool,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionalDelta {
    pub leader: LeaderId,
    pub asset: AssetId,
    pub signed_risk_usdc: i64,
    pub leader_event_ts_ms: UnixMs,
    pub local_recv_ts_ms: UnixMs,
    pub expires_at_ms: UnixMs,
    pub tx_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityTrigger {
    pub asset: AssetId,
    pub side: String,
    pub outcome: String,
    pub usdc_size: UsdcMicros,
    pub leader_event_ts_ms: UnixMs,
    pub local_recv_ts_ms: UnixMs,
    pub tx_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookLevel {
    pub price_ppm: PricePpm,
    pub size_shares: SharesMicros,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookView {
    pub asset: AssetId,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub tick_size_ppm: PricePpm,
    pub min_order_size_shares: SharesMicros,
    pub last_trade_price_ppm: PricePpm,
    pub last_update_ms: UnixMs,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketMeta {
    pub asset: AssetId,
    pub condition: ConditionId,
    pub event: Option<EventId>,
    pub end_ts_ms: UnixMs,
    pub accepting_orders: bool,
    pub enable_order_book: bool,
    pub liquidity_clob: UsdcMicros,
    pub volume_24h_clob: UsdcMicros,
    pub neg_risk: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnPosition {
    pub asset: AssetId,
    pub shares: SharesMicros,
    pub avg_price_ppm: PricePpm,
    pub risk_usdc: UsdcMicros,
    pub current_value: UsdcMicros,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub asset: AssetId,
    pub signed_target_risk_usdc: i64,
    pub confidence_bps: u16,
    pub source_count: u8,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TteBucket {
    Under24h,
    Under72h,
    Over72h,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredDelta {
    pub asset: AssetId,
    pub current_risk_usdc: i64,
    pub target_risk_usdc: i64,
    pub delta_risk_usdc: i64,
    pub confidence_bps: u16,
    pub max_copy_gap_bps: u16,
    pub max_slip_bps: u16,
    pub tte_bucket: TteBucket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    pub aggregated_assets: usize,
    pub stale_assets: Vec<AssetId>,
    pub blocked_assets: Vec<(AssetId, String)>,
    pub total_target_risk_usdc: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizingOutput {
    pub targets: Vec<Target>,
    pub deltas: Vec<DesiredDelta>,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone)]
pub struct StrategyConfig {
    pub total_risk_budget_bps: u32,
    pub per_market_cap_bps: u32,
    pub per_event_cap_bps: u32,
    pub global_risk_scale_bps: u32,
    pub lag_tau_ms: UnixMs,
    pub stale_tau_ms: UnixMs,
    pub signal_tau_ms: UnixMs,
    pub fast_tau_ms: UnixMs,
    pub agreement_exponent: f64,
    pub correlation_lambda: f64,
    pub no_new_position_before_ms: UnixMs,
    pub reduce_size_before_ms: UnixMs,
    pub block_neg_risk_entries: bool,
    pub min_copyable_liquidity_usdc: UsdcMicros,
    pub min_copyable_volume_usdc: UsdcMicros,
    pub max_copy_gap_bps: u16,
    pub max_slip_bps: u16,
    pub min_effective_order_usdc: UsdcMicros,
    pub fee_rate_bps: u16,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            total_risk_budget_bps: 3_000,
            per_market_cap_bps: 200,
            per_event_cap_bps: 600,
            global_risk_scale_bps: 6_000,
            lag_tau_ms: 3_000,
            stale_tau_ms: 90_000,
            signal_tau_ms: 60_000,
            fast_tau_ms: 30_000,
            agreement_exponent: 1.5,
            correlation_lambda: 0.5,
            no_new_position_before_ms: 24 * 60 * 60 * 1000,
            reduce_size_before_ms: 72 * 60 * 60 * 1000,
            block_neg_risk_entries: true,
            min_copyable_liquidity_usdc: 50_000_000_000,
            min_copyable_volume_usdc: 20_000_000_000,
            max_copy_gap_bps: 80,
            max_slip_bps: 60,
            min_effective_order_usdc: 20_000_000,
            fee_rate_bps: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeaderState {
    pub config: LeaderConfig,
    pub value: LeaderValue,
    pub positions: Vec<LeaderPosition>,
    pub provisional_deltas: Vec<ProvisionalDelta>,
    pub lag_ms: UnixMs,
    pub stale_ms: UnixMs,
    pub avg_corr_bps: u16,
}

#[derive(Debug)]
pub struct SizingInput<'a> {
    pub now_ms: UnixMs,
    pub equity_usdc: UsdcMicros,
    pub leaders: &'a [LeaderState],
    pub books: &'a HashMap<AssetId, BookView>,
    pub metas: &'a HashMap<AssetId, MarketMeta>,
    pub own_positions: &'a HashMap<AssetId, OwnPosition>,
    pub cfg: &'a StrategyConfig,
}

#[derive(Debug, Clone)]
struct ExposureAggregate {
    weighted_sum: f64,
    weight_sum: f64,
    sign_sum: f64,
    sign_weight_sum: f64,
    freshest_signal_ms: Option<UnixMs>,
    latest_price_ppm: Option<PricePpm>,
    source_count: u8,
    max_leader_share: f64,
    leader_contributions: HashMap<LeaderId, f64>,
}

impl Default for ExposureAggregate {
    fn default() -> Self {
        Self {
            weighted_sum: 0.0,
            weight_sum: 0.0,
            sign_sum: 0.0,
            sign_weight_sum: 0.0,
            freshest_signal_ms: None,
            latest_price_ppm: None,
            source_count: 0,
            max_leader_share: 0.0,
            leader_contributions: HashMap::new(),
        }
    }
}

pub fn compute_targets(input: &SizingInput<'_>) -> SizingOutput {
    let aggregates = collect_leader_exposures(input.leaders, input.now_ms, input.cfg);
    let mut raw_targets =
        aggregate_exposures(&aggregates, input.equity_usdc, input.cfg, input.metas);
    apply_market_gating(
        &mut raw_targets,
        input.own_positions,
        input.metas,
        input.now_ms,
        input.cfg,
    );
    let projected = project_constraints(
        raw_targets,
        input.own_positions,
        input.metas,
        input.equity_usdc,
        input.cfg,
    );
    let liquid = project_liquidity(projected, &aggregates, input.books, input.cfg);
    let deltas = build_deltas(
        &liquid,
        input.own_positions,
        input.metas,
        input.now_ms,
        input.cfg,
    );

    let stale_assets = liquid
        .iter()
        .filter(|target| target.stale)
        .map(|target| target.asset.clone())
        .collect::<Vec<_>>();
    let blocked_assets = liquid
        .iter()
        .filter(|target| target.signed_target_risk_usdc == 0)
        .filter_map(|target| {
            let current = current_risk(input.own_positions, &target.asset);
            (current == 0).then(|| (target.asset.clone(), "projected_to_zero".to_string()))
        })
        .collect::<Vec<_>>();
    let total_target_risk_usdc = liquid
        .iter()
        .map(|target| target.signed_target_risk_usdc)
        .sum();

    SizingOutput {
        targets: liquid,
        deltas,
        diagnostics: Diagnostics {
            aggregated_assets: aggregates.len(),
            stale_assets,
            blocked_assets,
            total_target_risk_usdc,
        },
    }
}

pub fn provisional_delta_from_activity(
    leader: &LeaderState,
    trigger: ActivityTrigger,
    cfg: &StrategyConfig,
) -> ProvisionalDelta {
    let sign = signed_direction(&trigger.side, &trigger.outcome);
    let decay = decay_factor_bps(
        trigger
            .local_recv_ts_ms
            .saturating_sub(trigger.leader_event_ts_ms),
        cfg.fast_tau_ms,
    );
    let signed_risk_usdc =
        ((sign as i128) * trigger.usdc_size as i128 * decay as i128 / 10_000) as i64;
    ProvisionalDelta {
        leader: leader.config.leader.clone(),
        asset: trigger.asset,
        signed_risk_usdc,
        leader_event_ts_ms: trigger.leader_event_ts_ms,
        local_recv_ts_ms: trigger.local_recv_ts_ms,
        expires_at_ms: trigger.local_recv_ts_ms.saturating_add(cfg.fast_tau_ms),
        tx_hash: trigger.tx_hash,
    }
}

fn collect_leader_exposures(
    leaders: &[LeaderState],
    now_ms: UnixMs,
    cfg: &StrategyConfig,
) -> HashMap<AssetId, ExposureAggregate> {
    let mut aggregates = HashMap::<AssetId, ExposureAggregate>::new();
    for leader in leaders.iter().filter(|leader| leader.config.enabled) {
        let weight = leader_effective_weight(leader, cfg);
        if weight <= 0.0 || leader.value.ewma_value <= 0 {
            continue;
        }
        for position in &leader.positions {
            let exposure = signed_position_exposure(position, leader.value.ewma_value);
            accumulate_exposure(
                &mut aggregates,
                &leader.config.leader,
                &position.asset,
                weight,
                exposure,
                Some(leader.value.last_update_ms),
                Some(position.avg_price_ppm),
            );
        }
        for provisional in &leader.provisional_deltas {
            if provisional.expires_at_ms < now_ms {
                continue;
            }
            let decay = decay_factor_f64(
                now_ms.saturating_sub(provisional.local_recv_ts_ms),
                cfg.fast_tau_ms,
            );
            let exposure =
                provisional.signed_risk_usdc as f64 / leader.value.ewma_value.max(1) as f64 * decay;
            accumulate_exposure(
                &mut aggregates,
                &leader.config.leader,
                &provisional.asset,
                weight,
                exposure,
                Some(provisional.local_recv_ts_ms),
                None,
            );
        }
    }

    for aggregate in aggregates.values_mut() {
        let total_contribution = aggregate
            .leader_contributions
            .values()
            .copied()
            .sum::<f64>();
        if total_contribution > 0.0 {
            aggregate.max_leader_share = aggregate
                .leader_contributions
                .values()
                .copied()
                .map(|value| value / total_contribution)
                .fold(0.0, f64::max);
        }
    }
    aggregates
}

fn accumulate_exposure(
    aggregates: &mut HashMap<AssetId, ExposureAggregate>,
    leader_id: &LeaderId,
    asset: &AssetId,
    weight: f64,
    exposure: f64,
    freshest_signal_ms: Option<UnixMs>,
    price_ppm: Option<PricePpm>,
) {
    if exposure.abs() <= DEFAULT_EXPOSURE_EPSILON {
        return;
    }
    let aggregate = aggregates.entry(asset.clone()).or_default();
    aggregate.weighted_sum += weight * exposure;
    aggregate.weight_sum += weight;
    aggregate.source_count = aggregate.source_count.saturating_add(1);
    if exposure.abs() > DEFAULT_EXPOSURE_EPSILON {
        aggregate.sign_sum += weight * exposure.signum();
        aggregate.sign_weight_sum += weight;
    }
    if let Some(ts) = freshest_signal_ms {
        aggregate.freshest_signal_ms = Some(
            aggregate
                .freshest_signal_ms
                .map_or(ts, |current| current.max(ts)),
        );
    }
    if let Some(price_ppm) = price_ppm {
        aggregate.latest_price_ppm = Some(price_ppm);
    }
    *aggregate
        .leader_contributions
        .entry(leader_id.clone())
        .or_insert(0.0) += (weight * exposure).abs();
}

fn aggregate_exposures(
    aggregates: &HashMap<AssetId, ExposureAggregate>,
    equity_usdc: UsdcMicros,
    cfg: &StrategyConfig,
    metas: &HashMap<AssetId, MarketMeta>,
) -> Vec<Target> {
    let mut out = Vec::new();
    for (asset, aggregate) in aggregates {
        if aggregate.weight_sum <= 0.0 {
            continue;
        }
        let z_bar = aggregate.weighted_sum / aggregate.weight_sum;
        let agreement = if aggregate.sign_weight_sum <= 0.0 {
            0.0
        } else {
            (aggregate.sign_sum.abs() / aggregate.sign_weight_sum).clamp(0.0, 1.0)
        };
        let fresh = aggregate
            .freshest_signal_ms
            .map(|ts| {
                decay_factor_f64(
                    0.max(
                        aggregate
                            .freshest_signal_ms
                            .unwrap_or(ts)
                            .saturating_sub(ts),
                    ),
                    cfg.signal_tau_ms,
                )
            })
            .unwrap_or(1.0);
        let meta = metas.get(asset);
        let expiry = meta
            .map(|meta| expiry_factor(meta.end_ts_ms, cfg))
            .unwrap_or(0.0);
        let copyable = meta.map(|meta| copyable_factor(meta, cfg)).unwrap_or(0.0);
        let max_share_scale = if aggregate.max_leader_share > 0.5 {
            0.5 / aggregate.max_leader_share
        } else {
            1.0
        };
        let raw_weight = (cfg.global_risk_scale_bps as f64 / 10_000.0)
            * z_bar
            * agreement.powf(cfg.agreement_exponent)
            * fresh
            * expiry
            * copyable
            * max_share_scale;
        let signed_target_risk_usdc = (equity_usdc as f64 * raw_weight).round() as i64;
        out.push(Target {
            asset: asset.clone(),
            signed_target_risk_usdc,
            confidence_bps: (agreement * 10_000.0).round().clamp(0.0, 10_000.0) as u16,
            source_count: aggregate.source_count,
            stale: fresh < 0.2,
        });
    }
    out.sort_by_key(|target| target.asset.0.clone());
    out
}

fn apply_market_gating(
    targets: &mut [Target],
    own_positions: &HashMap<AssetId, OwnPosition>,
    metas: &HashMap<AssetId, MarketMeta>,
    now_ms: UnixMs,
    cfg: &StrategyConfig,
) {
    for target in targets {
        let Some(meta) = metas.get(&target.asset) else {
            target.signed_target_risk_usdc = 0;
            target.stale = true;
            continue;
        };
        if !meta.accepting_orders || !meta.enable_order_book {
            target.signed_target_risk_usdc = 0;
            target.stale = true;
            continue;
        }
        let current = current_risk(own_positions, &target.asset);
        let tte = meta.end_ts_ms.saturating_sub(now_ms);
        if tte < cfg.no_new_position_before_ms {
            target.signed_target_risk_usdc =
                clamp_toward_flat(target.signed_target_risk_usdc, current);
            target.stale = true;
        }
        if meta.neg_risk && cfg.block_neg_risk_entries {
            target.signed_target_risk_usdc =
                clamp_toward_flat(target.signed_target_risk_usdc, current);
            target.stale = true;
        }
    }
}

fn project_constraints(
    mut targets: Vec<Target>,
    own_positions: &HashMap<AssetId, OwnPosition>,
    metas: &HashMap<AssetId, MarketMeta>,
    equity_usdc: UsdcMicros,
    cfg: &StrategyConfig,
) -> Vec<Target> {
    let per_market_cap = (equity_usdc as i128 * cfg.per_market_cap_bps as i128 / 10_000) as i64;
    let per_event_cap = (equity_usdc as i128 * cfg.per_event_cap_bps as i128 / 10_000) as i64;
    let total_budget = (equity_usdc as i128 * cfg.total_risk_budget_bps as i128 / 10_000) as i64;

    for target in &mut targets {
        target.signed_target_risk_usdc = target
            .signed_target_risk_usdc
            .clamp(-per_market_cap, per_market_cap);
    }

    let mut by_event = BTreeMap::<Option<EventId>, Vec<usize>>::new();
    for (index, target) in targets.iter().enumerate() {
        let event = metas.get(&target.asset).and_then(|meta| meta.event.clone());
        by_event.entry(event).or_default().push(index);
    }
    for indexes in by_event.values() {
        let total = indexes
            .iter()
            .map(|index| targets[*index].signed_target_risk_usdc.abs())
            .sum::<i64>();
        if total > per_event_cap && total > 0 {
            let scale = per_event_cap as f64 / total as f64;
            for index in indexes {
                let value = targets[*index].signed_target_risk_usdc as f64 * scale;
                targets[*index].signed_target_risk_usdc = value.round() as i64;
            }
        }
    }

    let total_abs = targets
        .iter()
        .map(|target| target.signed_target_risk_usdc.abs())
        .sum::<i64>();
    if total_abs > total_budget && total_abs > 0 {
        let scale = total_budget as f64 / total_abs as f64;
        for target in &mut targets {
            target.signed_target_risk_usdc =
                (target.signed_target_risk_usdc as f64 * scale).round() as i64;
        }
    }

    for target in &mut targets {
        let current = current_risk(own_positions, &target.asset);
        if target.signed_target_risk_usdc.signum() != current.signum() && current != 0 {
            target.signed_target_risk_usdc =
                clamp_toward_flat(target.signed_target_risk_usdc, current);
        }
    }

    targets
}

fn project_liquidity(
    targets: Vec<Target>,
    aggregates: &HashMap<AssetId, ExposureAggregate>,
    books: &HashMap<AssetId, BookView>,
    cfg: &StrategyConfig,
) -> Vec<Target> {
    let mut out = Vec::with_capacity(targets.len());
    for mut target in targets {
        let Some(book) = books.get(&target.asset) else {
            target.signed_target_risk_usdc = 0;
            target.stale = true;
            out.push(target);
            continue;
        };
        let leader_price = aggregates
            .get(&target.asset)
            .and_then(|aggregate| aggregate.latest_price_ppm)
            .unwrap_or(book.last_trade_price_ppm);
        let max_exec = if target.signed_target_risk_usdc >= 0 {
            max_buyable_risk_usdc(
                book,
                leader_price,
                target.signed_target_risk_usdc.abs(),
                cfg,
            )
        } else {
            max_sellable_risk_usdc(
                book,
                leader_price,
                target.signed_target_risk_usdc.abs(),
                cfg,
            )
        };
        let capped = target.signed_target_risk_usdc.abs().min(max_exec.abs());
        target.signed_target_risk_usdc = capped * target.signed_target_risk_usdc.signum();
        out.push(target);
    }
    out
}

fn build_deltas(
    targets: &[Target],
    own_positions: &HashMap<AssetId, OwnPosition>,
    metas: &HashMap<AssetId, MarketMeta>,
    now_ms: UnixMs,
    cfg: &StrategyConfig,
) -> Vec<DesiredDelta> {
    targets
        .iter()
        .filter_map(|target| {
            let current = current_risk(own_positions, &target.asset);
            let delta = target.signed_target_risk_usdc - current;
            if delta.abs() < cfg.min_effective_order_usdc {
                return None;
            }
            let tte_bucket = metas
                .get(&target.asset)
                .map(|meta| bucket_tte(meta.end_ts_ms.saturating_sub(now_ms), cfg))
                .unwrap_or(TteBucket::Under24h);
            Some(DesiredDelta {
                asset: target.asset.clone(),
                current_risk_usdc: current,
                target_risk_usdc: target.signed_target_risk_usdc,
                delta_risk_usdc: delta,
                confidence_bps: target.confidence_bps,
                max_copy_gap_bps: cfg.max_copy_gap_bps,
                max_slip_bps: cfg.max_slip_bps,
                tte_bucket,
            })
        })
        .collect()
}

fn leader_effective_weight(leader: &LeaderState, cfg: &StrategyConfig) -> f64 {
    let base = leader.config.base_score_bps as f64 / 10_000.0;
    let alpha = leader.config.alpha_bps as f64 / 10_000.0;
    let lag_decay = decay_factor_f64(leader.lag_ms, cfg.lag_tau_ms);
    let stale_decay = decay_factor_f64(leader.stale_ms, cfg.stale_tau_ms);
    let corr_penalty =
        1.0 / (1.0 + cfg.correlation_lambda * (leader.avg_corr_bps as f64 / 10_000.0));
    base * alpha * lag_decay * stale_decay * corr_penalty
}

fn signed_position_exposure(position: &LeaderPosition, ewma_value: UsdcMicros) -> f64 {
    let sign = match position.outcome.to_ascii_uppercase().as_str() {
        "NO" => -1.0,
        _ => 1.0,
    };
    let risk = position_risk_usdc(position).max(0) as f64;
    sign * risk / ewma_value.max(1) as f64
}

fn position_risk_usdc(position: &LeaderPosition) -> UsdcMicros {
    if position.initial_value > 0 {
        position.initial_value
    } else {
        ((position.size as i128 * position.avg_price_ppm as i128) / 1_000_000) as i64
    }
}

fn signed_direction(side: &str, outcome: &str) -> i64 {
    match (
        side.to_ascii_uppercase().as_str(),
        outcome.to_ascii_uppercase().as_str(),
    ) {
        ("BUY", "YES") => 1,
        ("SELL", "YES") => -1,
        ("BUY", "NO") => -1,
        ("SELL", "NO") => 1,
        ("BUY", _) => 1,
        ("SELL", _) => -1,
        _ => 0,
    }
}

fn decay_factor_bps(elapsed_ms: UnixMs, tau_ms: UnixMs) -> u32 {
    (decay_factor_f64(elapsed_ms, tau_ms) * 10_000.0).round() as u32
}

fn decay_factor_f64(elapsed_ms: UnixMs, tau_ms: UnixMs) -> f64 {
    if tau_ms <= 0 {
        return 0.0;
    }
    (-(elapsed_ms.max(0) as f64) / tau_ms as f64)
        .exp()
        .clamp(0.0, 1.0)
}

fn expiry_factor(end_ts_ms: UnixMs, cfg: &StrategyConfig) -> f64 {
    let tte = end_ts_ms.max(0);
    if tte <= cfg.no_new_position_before_ms {
        0.0
    } else if tte < cfg.reduce_size_before_ms {
        0.5
    } else {
        1.0
    }
}

fn copyable_factor(meta: &MarketMeta, cfg: &StrategyConfig) -> f64 {
    if !meta.accepting_orders || !meta.enable_order_book {
        return 0.0;
    }
    let liq = log_scaled(
        meta.liquidity_clob,
        cfg.min_copyable_liquidity_usdc,
        cfg.min_copyable_liquidity_usdc * 10,
    );
    let vol = log_scaled(
        meta.volume_24h_clob,
        cfg.min_copyable_volume_usdc,
        cfg.min_copyable_volume_usdc * 10,
    );
    liq * vol
}

fn log_scaled(value: UsdcMicros, min: UsdcMicros, max: UsdcMicros) -> f64 {
    if value <= 0 || max <= min {
        return 0.0;
    }
    let value = (value as f64 + 1.0).ln();
    let min = (min as f64 + 1.0).ln();
    let max = (max as f64 + 1.0).ln();
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

fn current_risk(own_positions: &HashMap<AssetId, OwnPosition>, asset: &AssetId) -> i64 {
    own_positions
        .get(asset)
        .map(|position| position.risk_usdc)
        .unwrap_or(0)
}

fn clamp_toward_flat(target: i64, current: i64) -> i64 {
    if current == 0 {
        0
    } else if target.signum() == current.signum() {
        if target.abs() > current.abs() {
            current
        } else {
            target
        }
    } else {
        0
    }
}

fn bucket_tte(tte_ms: UnixMs, cfg: &StrategyConfig) -> TteBucket {
    if tte_ms < cfg.no_new_position_before_ms {
        TteBucket::Under24h
    } else if tte_ms < cfg.reduce_size_before_ms {
        TteBucket::Under72h
    } else {
        TteBucket::Over72h
    }
}

fn max_buyable_risk_usdc(
    book: &BookView,
    leader_price_ppm: PricePpm,
    desired_risk_usdc: i64,
    cfg: &StrategyConfig,
) -> i64 {
    let mut spent: i64 = 0;
    let best_ask = book.asks.first().map(|level| level.price_ppm).unwrap_or(0);
    if best_ask <= 0 {
        return 0;
    }
    for level in &book.asks {
        if level.size_shares <= 0 || level.price_ppm <= 0 {
            continue;
        }
        let slip_bps = price_gap_bps(level.price_ppm, best_ask);
        let copy_gap_bps = price_gap_bps(level.price_ppm, leader_price_ppm.max(1));
        if slip_bps > cfg.max_slip_bps as i64 || copy_gap_bps > cfg.max_copy_gap_bps as i64 {
            break;
        }
        let level_notional = shares_to_usdc(level.size_shares, level.price_ppm);
        let remaining = desired_risk_usdc - spent;
        if remaining <= 0 {
            break;
        }
        spent += level_notional.min(remaining);
    }
    spent.max(0)
}

fn max_sellable_risk_usdc(
    book: &BookView,
    leader_price_ppm: PricePpm,
    desired_risk_usdc: i64,
    cfg: &StrategyConfig,
) -> i64 {
    let mut recv: i64 = 0;
    let best_bid = book.bids.first().map(|level| level.price_ppm).unwrap_or(0);
    if best_bid <= 0 {
        return 0;
    }
    for level in &book.bids {
        if level.size_shares <= 0 || level.price_ppm <= 0 {
            continue;
        }
        let slip_bps = price_gap_bps(level.price_ppm, best_bid);
        let copy_gap_bps = price_gap_bps(level.price_ppm, leader_price_ppm.max(1));
        if slip_bps > cfg.max_slip_bps as i64 || copy_gap_bps > cfg.max_copy_gap_bps as i64 {
            break;
        }
        let level_notional = shares_to_usdc(level.size_shares, level.price_ppm);
        let remaining = desired_risk_usdc - recv;
        if remaining <= 0 {
            break;
        }
        recv += level_notional.min(remaining);
    }
    recv.max(0)
}

fn price_gap_bps(price_a_ppm: PricePpm, price_b_ppm: PricePpm) -> i64 {
    if price_b_ppm <= 0 {
        return i64::MAX;
    }
    let ratio = ((price_a_ppm as f64 - price_b_ppm as f64).abs() / price_b_ppm as f64) * 10_000.0;
    ratio.round() as i64
}

fn shares_to_usdc(shares: SharesMicros, price_ppm: PricePpm) -> i64 {
    ((shares as i128 * price_ppm as i128) / 1_000_000) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(id: &str) -> AssetId {
        AssetId(id.to_string())
    }
    fn condition(id: &str) -> ConditionId {
        ConditionId(id.to_string())
    }
    fn event(id: &str) -> EventId {
        EventId(id.to_string())
    }
    fn leader(id: &str) -> LeaderId {
        LeaderId(id.to_string())
    }

    fn leader_state(
        id: &str,
        base_score_bps: u32,
        alpha_bps: u32,
        ewma_value: UsdcMicros,
        positions: Vec<LeaderPosition>,
    ) -> LeaderState {
        LeaderState {
            config: LeaderConfig {
                leader: leader(id),
                base_score_bps,
                alpha_bps,
                enabled: true,
            },
            value: LeaderValue {
                spot_value: ewma_value,
                ewma_value,
                last_update_ms: 1_000,
            },
            positions,
            provisional_deltas: Vec::new(),
            lag_ms: 100,
            stale_ms: 100,
            avg_corr_bps: 1_000,
        }
    }

    fn market_meta(asset_id: &str, event_id: &str, end_ts_ms: UnixMs) -> MarketMeta {
        MarketMeta {
            asset: asset(asset_id),
            condition: condition(&format!("cond-{asset_id}")),
            event: Some(event(event_id)),
            end_ts_ms,
            accepting_orders: true,
            enable_order_book: true,
            liquidity_clob: 200_000_000_000,
            volume_24h_clob: 100_000_000_000,
            neg_risk: false,
        }
    }

    fn book(asset_id: &str) -> BookView {
        BookView {
            asset: asset(asset_id),
            bids: vec![BookLevel {
                price_ppm: 480_000,
                size_shares: 500_000_000,
            }],
            asks: vec![BookLevel {
                price_ppm: 500_000,
                size_shares: 500_000_000,
            }],
            tick_size_ppm: 1_000,
            min_order_size_shares: 10_000,
            last_trade_price_ppm: 500_000,
            last_update_ms: 1_000,
            hash: "book-1".into(),
        }
    }

    #[test]
    fn provisional_delta_respects_yes_no_signs() {
        let state = leader_state("leader-1", 9_000, 3_000, 100_000_000, vec![]);
        let buy_yes = provisional_delta_from_activity(
            &state,
            ActivityTrigger {
                asset: asset("asset-1"),
                side: "BUY".into(),
                outcome: "YES".into(),
                usdc_size: 20_000_000,
                leader_event_ts_ms: 1_000,
                local_recv_ts_ms: 1_200,
                tx_hash: "0xtx".into(),
            },
            &StrategyConfig::default(),
        );
        let buy_no = provisional_delta_from_activity(
            &state,
            ActivityTrigger {
                asset: asset("asset-1"),
                side: "BUY".into(),
                outcome: "NO".into(),
                usdc_size: 20_000_000,
                leader_event_ts_ms: 1_000,
                local_recv_ts_ms: 1_200,
                tx_hash: "0xtx".into(),
            },
            &StrategyConfig::default(),
        );
        assert!(buy_yes.signed_risk_usdc > 0);
        assert!(buy_no.signed_risk_usdc < 0);
    }

    #[test]
    fn compute_targets_builds_positive_target_and_delta() {
        let mut metas = HashMap::new();
        metas.insert(
            asset("asset-1"),
            market_meta("asset-1", "event-1", 500_000_000),
        );
        let mut books = HashMap::new();
        books.insert(asset("asset-1"), book("asset-1"));
        let leaders = vec![leader_state(
            "leader-1",
            9_000,
            3_000,
            100_000_000,
            vec![LeaderPosition {
                asset: asset("asset-1"),
                condition: condition("cond-1"),
                event: Some(event("event-1")),
                outcome: "YES".into(),
                size: 100_000_000,
                avg_price_ppm: 500_000,
                initial_value: 50_000_000,
                current_value: 55_000_000,
                end_ts_ms: 500_000_000,
                neg_risk: false,
                slug: "market-1".into(),
            }],
        )];
        let input = SizingInput {
            now_ms: 1_000,
            equity_usdc: 1_000_000_000,
            leaders: &leaders,
            books: &books,
            metas: &metas,
            own_positions: &HashMap::new(),
            cfg: &StrategyConfig::default(),
        };
        let out = compute_targets(&input);
        assert_eq!(out.targets.len(), 1);
        assert!(out.targets[0].signed_target_risk_usdc > 0);
        assert_eq!(out.deltas.len(), 1);
        assert!(out.deltas[0].delta_risk_usdc > 0);
    }

    #[test]
    fn compute_targets_blocks_new_entries_inside_24h_and_neg_risk() {
        let mut metas = HashMap::new();
        metas.insert(
            asset("asset-1"),
            MarketMeta {
                neg_risk: true,
                end_ts_ms: 1_000 + 23 * 60 * 60 * 1000,
                ..market_meta("asset-1", "event-1", 1_000 + 23 * 60 * 60 * 1000)
            },
        );
        let mut books = HashMap::new();
        books.insert(asset("asset-1"), book("asset-1"));
        let leaders = vec![leader_state(
            "leader-1",
            9_000,
            3_000,
            100_000_000,
            vec![LeaderPosition {
                asset: asset("asset-1"),
                condition: condition("cond-1"),
                event: Some(event("event-1")),
                outcome: "YES".into(),
                size: 100_000_000,
                avg_price_ppm: 500_000,
                initial_value: 50_000_000,
                current_value: 55_000_000,
                end_ts_ms: 1_000 + 23 * 60 * 60 * 1000,
                neg_risk: true,
                slug: "market-1".into(),
            }],
        )];
        let input = SizingInput {
            now_ms: 1_000,
            equity_usdc: 1_000_000_000,
            leaders: &leaders,
            books: &books,
            metas: &metas,
            own_positions: &HashMap::new(),
            cfg: &StrategyConfig::default(),
        };
        let out = compute_targets(&input);
        assert_eq!(out.targets[0].signed_target_risk_usdc, 0);
        assert!(out.targets[0].stale);
        assert!(out.deltas.is_empty());
    }

    #[test]
    fn compute_targets_respects_event_cap_and_market_cap() {
        let cfg = StrategyConfig {
            per_market_cap_bps: 100,
            per_event_cap_bps: 150,
            ..StrategyConfig::default()
        };
        let mut metas = HashMap::new();
        metas.insert(
            asset("asset-1"),
            market_meta("asset-1", "event-1", 500_000_000),
        );
        metas.insert(
            asset("asset-2"),
            market_meta("asset-2", "event-1", 500_000_000),
        );
        let mut books = HashMap::new();
        books.insert(asset("asset-1"), book("asset-1"));
        books.insert(asset("asset-2"), book("asset-2"));
        let leaders = vec![leader_state(
            "leader-1",
            10_000,
            10_000,
            100_000_000,
            vec![
                LeaderPosition {
                    asset: asset("asset-1"),
                    condition: condition("cond-1"),
                    event: Some(event("event-1")),
                    outcome: "YES".into(),
                    size: 100_000_000,
                    avg_price_ppm: 500_000,
                    initial_value: 80_000_000,
                    current_value: 85_000_000,
                    end_ts_ms: 500_000_000,
                    neg_risk: false,
                    slug: "market-1".into(),
                },
                LeaderPosition {
                    asset: asset("asset-2"),
                    condition: condition("cond-2"),
                    event: Some(event("event-1")),
                    outcome: "YES".into(),
                    size: 100_000_000,
                    avg_price_ppm: 500_000,
                    initial_value: 80_000_000,
                    current_value: 85_000_000,
                    end_ts_ms: 500_000_000,
                    neg_risk: false,
                    slug: "market-2".into(),
                },
            ],
        )];
        let out = compute_targets(&SizingInput {
            now_ms: 1_000,
            equity_usdc: 1_000_000_000,
            leaders: &leaders,
            books: &books,
            metas: &metas,
            own_positions: &HashMap::new(),
            cfg: &cfg,
        });
        assert!(
            out.targets
                .iter()
                .all(|target| target.signed_target_risk_usdc.abs() <= 100_000_000)
        );
        let total_event = out
            .targets
            .iter()
            .map(|target| target.signed_target_risk_usdc.abs())
            .sum::<i64>();
        assert!(total_event <= 150_000_000);
    }

    #[test]
    fn project_liquidity_caps_targets_by_book_depth() {
        let cfg = StrategyConfig {
            max_slip_bps: 20,
            ..StrategyConfig::default()
        };
        let mut metas = HashMap::new();
        metas.insert(
            asset("asset-1"),
            market_meta("asset-1", "event-1", 500_000_000),
        );
        let mut books = HashMap::new();
        books.insert(
            asset("asset-1"),
            BookView {
                asks: vec![BookLevel {
                    price_ppm: 520_000,
                    size_shares: 10_000_000,
                }],
                ..book("asset-1")
            },
        );
        let leaders = vec![leader_state(
            "leader-1",
            10_000,
            10_000,
            100_000_000,
            vec![LeaderPosition {
                asset: asset("asset-1"),
                condition: condition("cond-1"),
                event: Some(event("event-1")),
                outcome: "YES".into(),
                size: 100_000_000,
                avg_price_ppm: 500_000,
                initial_value: 90_000_000,
                current_value: 90_000_000,
                end_ts_ms: 500_000_000,
                neg_risk: false,
                slug: "market-1".into(),
            }],
        )];
        let out = compute_targets(&SizingInput {
            now_ms: 1_000,
            equity_usdc: 1_000_000_000,
            leaders: &leaders,
            books: &books,
            metas: &metas,
            own_positions: &HashMap::new(),
            cfg: &cfg,
        });
        assert!(out.targets[0].signed_target_risk_usdc <= 5_200_000);
    }
}
