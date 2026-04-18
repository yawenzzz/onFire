use super::event::{MonEvent, RejectReason, Side, SkipReason, Svc, WsCh};
use super::hist::{RollingHistogram, RollingRms};
use super::rolling::RollingCounter;
use super::snapshot::{
    AlertView, BookViewUi, ExecView, FeedChannelView, FeedHttpView, FeedView, Health, LeaderView,
    Mode, PositionTargetingView, ProcView, RiskView, SelectedLeaderView, SignalView,
    TrackedActivityView, TradeTapeView, UiSnapshot,
};
use super::{MonitorCfg, now_ms_u64};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone)]
struct ProcState {
    started_ms: u64,
    loop_lag_ms: RollingHistogram,
    cpu_tenths_pct: u16,
    rss_mb: u64,
    open_fds: u64,
    threads: u64,
    build_label: String,
}

impl ProcState {
    fn new(now_ms: u64) -> Self {
        Self {
            started_ms: now_ms,
            loop_lag_ms: RollingHistogram::new(60_000),
            cpu_tenths_pct: 0,
            rss_mb: 0,
            open_fds: 0,
            threads: 0,
            build_label: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct HttpSvcState {
    latency_ms: RollingHistogram,
    req_1m: RollingCounter,
    http_429_1m: RollingCounter,
    http_5xx_1m: RollingCounter,
    backoff_active: bool,
    rl_fill_ratio_bps: u16,
}

impl HttpSvcState {
    fn new(now_ms: u64) -> Self {
        Self {
            latency_ms: RollingHistogram::new(300_000),
            req_1m: RollingCounter::new(1_000, 60, now_ms),
            http_429_1m: RollingCounter::new(1_000, 60, now_ms),
            http_5xx_1m: RollingCounter::new(1_000, 60, now_ms),
            backoff_active: false,
            rl_fill_ratio_bps: 10_000,
        }
    }

    fn apply(&mut self, now_ms: u64, status: u16, latency_ms: u32) {
        self.latency_ms.record(now_ms, latency_ms as u64);
        self.req_1m.incr(now_ms, 1);
        if status == 429 {
            self.http_429_1m.incr(now_ms, 1);
            self.rl_fill_ratio_bps = 500;
        } else if status >= 500 {
            self.http_5xx_1m.incr(now_ms, 1);
        } else if self.rl_fill_ratio_bps < 10_000 {
            self.rl_fill_ratio_bps = self.rl_fill_ratio_bps.saturating_add(250).min(10_000);
        }
    }

    fn view(&mut self, now_ms: u64) -> FeedHttpView {
        FeedHttpView {
            latency_p50_ms: self.latency_ms.p50(now_ms),
            latency_p95_ms: self.latency_ms.p95(now_ms),
            status_429_1m: self.http_429_1m.sum(now_ms),
            status_5xx_1m: self.http_5xx_1m.sum(now_ms),
            rl_fill_ratio_bps: self.rl_fill_ratio_bps,
            backoff_active: self.backoff_active,
        }
    }
}

#[derive(Debug, Clone)]
struct WsState {
    connected: bool,
    last_msg_ms: Option<u64>,
    pong_rtt_ms: RollingHistogram,
    reconnect_total: u64,
    decode_err_total: u64,
    note: Option<String>,
}

impl WsState {
    fn new() -> Self {
        Self {
            connected: false,
            last_msg_ms: None,
            pong_rtt_ms: RollingHistogram::new(300_000),
            reconnect_total: 0,
            decode_err_total: 0,
            note: Some("disabled".to_string()),
        }
    }

    fn view(&mut self, now_ms: u64) -> FeedChannelView {
        FeedChannelView {
            connected: self.connected,
            last_msg_age_ms: self
                .last_msg_ms
                .map(|ts| now_ms.saturating_sub(ts))
                .unwrap_or(0),
            pong_p95_ms: self.pong_rtt_ms.p95(now_ms),
            reconnect_total: self.reconnect_total,
            decode_err_total: self.decode_err_total,
            note: self.note.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct FeedState {
    market_ws: WsState,
    user_ws: WsState,
    data_api: HttpSvcState,
    gamma_api: HttpSvcState,
    clob_api: HttpSvcState,
}

impl FeedState {
    fn new(now_ms: u64) -> Self {
        Self {
            market_ws: WsState::new(),
            user_ws: WsState::new(),
            data_api: HttpSvcState::new(now_ms),
            gamma_api: HttpSvcState::new(now_ms),
            clob_api: HttpSvcState::new(now_ms),
        }
    }

    fn http_mut(&mut self, svc: Svc) -> &mut HttpSvcState {
        match svc {
            Svc::Data => &mut self.data_api,
            Svc::Gamma => &mut self.gamma_api,
            Svc::Clob | Svc::Local => &mut self.clob_api,
        }
    }

    fn ws_mut(&mut self, ch: WsCh) -> &mut WsState {
        match ch {
            WsCh::Market => &mut self.market_ws,
            WsCh::User => &mut self.user_ws,
        }
    }
}

#[derive(Debug, Clone)]
struct LeaderMon {
    activity_age_ms: RollingHistogram,
    reconcile_latency_ms: RollingHistogram,
    drift_bps: RollingHistogram,
    last_activity_ms: Option<u64>,
    last_tx: Option<String>,
    last_side: Option<String>,
    last_slug: Option<String>,
    snapshot_age_ms: u64,
    positions_count: u16,
    value_usdc: i64,
    dirty: bool,
}

impl LeaderMon {
    fn new() -> Self {
        Self {
            activity_age_ms: RollingHistogram::new(300_000),
            reconcile_latency_ms: RollingHistogram::new(300_000),
            drift_bps: RollingHistogram::new(300_000),
            last_activity_ms: None,
            last_tx: None,
            last_side: None,
            last_slug: None,
            snapshot_age_ms: 0,
            positions_count: 0,
            value_usdc: 0,
            dirty: false,
        }
    }

    fn view(&mut self, now_ms: u64, leader: &str) -> LeaderView {
        LeaderView {
            leader: leader.to_string(),
            activity_p95_ms: self.activity_age_ms.p95(now_ms),
            snap_age_ms: self.snapshot_age_ms,
            reconcile_p95_ms: self.reconcile_latency_ms.p95(now_ms),
            drift_p95_bps: self.drift_bps.p95(now_ms),
            dirty: self.dirty,
            positions_count: self.positions_count,
            value_usdc: self.value_usdc,
            last_tx: self.last_tx.clone(),
            last_side: self.last_side.clone(),
            last_slug: self.last_slug.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct AssetMon {
    age_ms: RollingHistogram,
    last_age_ms: u64,
    spread_bps: u64,
    levels_bid: u16,
    levels_ask: u16,
    resync_5m: RollingCounter,
    crossed: bool,
    hash_mismatch: bool,
}

impl AssetMon {
    fn new(now_ms: u64) -> Self {
        Self {
            age_ms: RollingHistogram::new(300_000),
            last_age_ms: 0,
            spread_bps: 0,
            levels_bid: 0,
            levels_ask: 0,
            resync_5m: RollingCounter::new(10_000, 30, now_ms),
            crossed: false,
            hash_mismatch: false,
        }
    }

    fn view(&mut self, now_ms: u64, asset: &str) -> BookViewUi {
        BookViewUi {
            asset: asset.to_string(),
            age_ms: self.last_age_ms.max(self.age_ms.p95(now_ms)),
            spread_bps: self.spread_bps,
            levels_bid: self.levels_bid,
            levels_ask: self.levels_ask,
            resync_5m: self.resync_5m.sum(now_ms),
            crossed: self.crossed,
            hash_mismatch: self.hash_mismatch,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SelectedLeaderMon {
    wallet: String,
    source: String,
    category: String,
    score: String,
    review_status: String,
    core_pool: String,
    active_pool: String,
}

#[derive(Debug, Clone, Default)]
struct TrackedActivityMon {
    tx: String,
    side: String,
    slug: String,
    asset: String,
    usdc_size: i64,
    price_ppm: i32,
    event_age_ms: u64,
    event_ts_ms: i64,
    local_time_gmt8: String,
    current_position_value_usdc: i64,
    current_position_size: i64,
    current_avg_price_ppm: i32,
    algo_target_risk_usdc: i64,
    algo_delta_risk_usdc: i64,
    algo_confidence_bps: u16,
    algo_tte_bucket: String,
    algo_reason: String,
}

#[derive(Debug, Clone, Default)]
struct TradeTapeMon {
    local_time_gmt8: String,
    tx: String,
    side: String,
    slug: String,
    asset: String,
    usdc_size: i64,
    price_ppm: i32,
    current_position_value_usdc: i64,
    algo_target_risk_usdc: i64,
    algo_delta_risk_usdc: i64,
    algo_reason: String,
}

#[derive(Debug, Clone)]
struct SignalMon {
    status: String,
    raw_target_usdc: i64,
    final_target_usdc: i64,
    agree_bps: u16,
    fresh_ms: u64,
    reason: Option<String>,
}

impl Default for SignalMon {
    fn default() -> Self {
        Self {
            status: "IDLE".to_string(),
            raw_target_usdc: 0,
            final_target_usdc: 0,
            agree_bps: 0,
            fresh_ms: 0,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PositionTargetingMon {
    target_count: u64,
    delta_count: u64,
    stale_asset_count: u64,
    blocked_asset_count: u64,
    blocker_summary: String,
}

#[derive(Debug, Clone)]
struct ExecState {
    activity_to_intent_ms: RollingHistogram,
    intent_to_post_ms: RollingHistogram,
    post_to_match_ms: RollingHistogram,
    match_to_confirm_ms: RollingHistogram,
    copy_gap_bps: RollingHistogram,
    slip_bps: RollingHistogram,
    fee_adj_slip_bps: RollingHistogram,
    fill_ratio_ppm: RollingHistogram,
    last_submit_status: String,
}

impl ExecState {
    fn new() -> Self {
        Self {
            activity_to_intent_ms: RollingHistogram::new(300_000),
            intent_to_post_ms: RollingHistogram::new(300_000),
            post_to_match_ms: RollingHistogram::new(300_000),
            match_to_confirm_ms: RollingHistogram::new(300_000),
            copy_gap_bps: RollingHistogram::new(300_000),
            slip_bps: RollingHistogram::new(300_000),
            fee_adj_slip_bps: RollingHistogram::new(300_000),
            fill_ratio_ppm: RollingHistogram::new(300_000),
            last_submit_status: "none".to_string(),
        }
    }

    fn view(&mut self, now_ms: u64) -> ExecView {
        ExecView {
            activity_to_intent_p95_ms: self.activity_to_intent_ms.p95(now_ms),
            intent_to_post_p95_ms: self.intent_to_post_ms.p95(now_ms),
            post_to_match_p95_ms: self.post_to_match_ms.p95(now_ms),
            match_to_confirm_p95_ms: self.match_to_confirm_ms.p95(now_ms),
            copy_gap_p95_bps: self.copy_gap_bps.p95(now_ms),
            slip_p95_bps: self.slip_bps.p95(now_ms),
            fee_adj_slip_p95_bps: self.fee_adj_slip_bps.p95(now_ms),
            fill_ratio_p50_ppm: self.fill_ratio_ppm.p50(now_ms),
            last_submit_status: self.last_submit_status.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct RiskState {
    current: RiskView,
    tracking_rmse: RollingRms,
}

impl RiskState {
    fn new() -> Self {
        Self {
            current: RiskView::default(),
            tracking_rmse: RollingRms::new(60_000),
        }
    }
}

pub struct MonState {
    cfg: MonitorCfg,
    mode: Mode,
    proc: ProcState,
    feeds: FeedState,
    leaders: BTreeMap<String, LeaderMon>,
    assets: BTreeMap<String, AssetMon>,
    signals: BTreeMap<String, SignalMon>,
    exec: ExecState,
    risk: RiskState,
    position_targeting: PositionTargetingMon,
    selected_leader: SelectedLeaderMon,
    tracked_activity: TrackedActivityMon,
    recent_trades: VecDeque<TradeTapeMon>,
    alerts: Vec<AlertView>,
    logs: VecDeque<String>,
}

impl MonState {
    pub fn new(cfg: MonitorCfg, mode: Mode) -> Self {
        let now_ms = now_ms_u64();
        Self {
            proc: ProcState::new(now_ms),
            feeds: FeedState::new(now_ms),
            leaders: BTreeMap::new(),
            assets: BTreeMap::new(),
            signals: BTreeMap::new(),
            exec: ExecState::new(),
            risk: RiskState::new(),
            position_targeting: PositionTargetingMon::default(),
            selected_leader: SelectedLeaderMon::default(),
            tracked_activity: TrackedActivityMon::default(),
            recent_trades: VecDeque::new(),
            alerts: Vec::new(),
            logs: VecDeque::new(),
            cfg,
            mode,
        }
    }

    fn push_log(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self
            .logs
            .back()
            .map(|line| line == &message)
            .unwrap_or(false)
        {
            return;
        }
        self.logs.push_back(message);
        while self.logs.len() > self.cfg.log_lines {
            self.logs.pop_front();
        }
    }

    fn leader_mut(&mut self, leader: &str) -> &mut LeaderMon {
        self.leaders
            .entry(leader.to_string())
            .or_insert_with(LeaderMon::new)
    }

    fn asset_mut(&mut self, asset: &str) -> &mut AssetMon {
        let now_ms = now_ms_u64();
        self.assets
            .entry(asset.to_string())
            .or_insert_with(|| AssetMon::new(now_ms))
    }

    pub fn apply(&mut self, ev: MonEvent) {
        let now_ms = now_ms_u64();
        match ev {
            MonEvent::LeaderSelected {
                wallet,
                source,
                category,
                score,
                review_status,
                core_pool,
                active_pool,
            } => {
                let changed = self.selected_leader.wallet != wallet
                    || self.selected_leader.category != category
                    || self.selected_leader.score != score
                    || self.selected_leader.review_status != review_status;
                self.selected_leader = SelectedLeaderMon {
                    wallet,
                    source,
                    category,
                    score,
                    review_status,
                    core_pool,
                    active_pool,
                };
                if changed {
                    self.push_log(format!(
                        "selected leader wallet={} category={} score={} review={}",
                        self.selected_leader.wallet,
                        self.selected_leader.category,
                        self.selected_leader.score,
                        self.selected_leader.review_status
                    ));
                }
            }
            MonEvent::HttpDone {
                svc,
                status,
                latency_ms,
                ..
            } => {
                self.feeds.http_mut(svc).apply(now_ms, status, latency_ms);
            }
            MonEvent::WsConnected { ch } => {
                let ws = self.feeds.ws_mut(ch);
                ws.connected = true;
                ws.note = Some("connected".to_string());
                ws.last_msg_ms = Some(now_ms);
            }
            MonEvent::WsDisconnected { ch, reason } => {
                let ws = self.feeds.ws_mut(ch);
                ws.connected = false;
                ws.reconnect_total = ws.reconnect_total.saturating_add(1);
                ws.note = Some(reason.clone());
                self.push_log(format!("ws {} down: {reason}", ch.as_str()));
            }
            MonEvent::WsPong { ch, rtt_ms } => {
                let ws = self.feeds.ws_mut(ch);
                ws.connected = true;
                ws.pong_rtt_ms.record(now_ms, rtt_ms as u64);
                ws.last_msg_ms = Some(now_ms);
                ws.note = Some("connected".to_string());
            }
            MonEvent::WsMsg { ch, recv_ts_ms, .. } => {
                let ws = self.feeds.ws_mut(ch);
                ws.connected = true;
                ws.last_msg_ms = Some(recv_ts_ms.max(0) as u64);
            }
            MonEvent::ActivityHit {
                leader,
                asset,
                side,
                usdc_size,
                leader_price_ppm,
                event_ts_ms,
                recv_ts_ms,
                tx_hash,
                slug,
                ..
            } => {
                let leader_mon = self.leader_mut(&leader);
                let age_ms = recv_ts_ms.saturating_sub(event_ts_ms).max(0) as u64;
                leader_mon.activity_age_ms.record(now_ms, age_ms);
                leader_mon.last_activity_ms = Some(recv_ts_ms.max(0) as u64);
                leader_mon.last_tx = Some(tx_hash.clone());
                leader_mon.last_side = Some(side.as_str().to_string());
                leader_mon.last_slug = slug.clone();
                let local_time_gmt8 = format_gmt8(event_ts_ms);
                self.tracked_activity = TrackedActivityMon {
                    tx: tx_hash.clone(),
                    side: side.as_str().to_string(),
                    slug: slug.unwrap_or_default(),
                    asset: asset.clone(),
                    usdc_size,
                    price_ppm: leader_price_ppm,
                    event_age_ms: age_ms,
                    event_ts_ms,
                    local_time_gmt8: local_time_gmt8.clone(),
                    current_position_value_usdc: 0,
                    current_position_size: 0,
                    current_avg_price_ppm: 0,
                    algo_target_risk_usdc: 0,
                    algo_delta_risk_usdc: 0,
                    algo_confidence_bps: 0,
                    algo_tte_bucket: String::new(),
                    algo_reason: String::new(),
                };
                self.recent_trades.push_back(TradeTapeMon {
                    local_time_gmt8,
                    tx: tx_hash.clone(),
                    side: side.as_str().to_string(),
                    slug: self.tracked_activity.slug.clone(),
                    asset: asset.clone(),
                    usdc_size,
                    price_ppm: leader_price_ppm,
                    current_position_value_usdc: 0,
                    algo_target_risk_usdc: 0,
                    algo_delta_risk_usdc: 0,
                    algo_reason: String::new(),
                });
                while self.recent_trades.len() > 8 {
                    self.recent_trades.pop_front();
                }
                self.push_log(format!(
                    "leader {leader} activity {} {} {}",
                    side.as_str(),
                    asset,
                    tx_hash
                ));
            }
            MonEvent::TrackedActivityProjection {
                asset,
                current_position_value_usdc,
                current_position_size,
                current_avg_price_ppm,
                algo_target_risk_usdc,
                algo_delta_risk_usdc,
                algo_confidence_bps,
                algo_tte_bucket,
                algo_reason,
                tracking_err_bps,
                follow_ratio_bps,
                copied_usdc,
                overcopy_usdc,
                undercopy_usdc,
            } => {
                if self.tracked_activity.asset == asset {
                    self.tracked_activity.current_position_value_usdc = current_position_value_usdc;
                    self.tracked_activity.current_position_size = current_position_size;
                    self.tracked_activity.current_avg_price_ppm = current_avg_price_ppm;
                    self.tracked_activity.algo_target_risk_usdc = algo_target_risk_usdc;
                    self.tracked_activity.algo_delta_risk_usdc = algo_delta_risk_usdc;
                    self.tracked_activity.algo_confidence_bps = algo_confidence_bps;
                    self.tracked_activity.algo_tte_bucket = algo_tte_bucket;
                    self.tracked_activity.algo_reason = algo_reason.clone();
                }
                self.risk.current.tracking_err_bps = tracking_err_bps;
                self.risk.current.follow_ratio_bps = follow_ratio_bps;
                self.risk.current.eligible_usdc = algo_target_risk_usdc.abs();
                self.risk.current.copied_usdc = copied_usdc;
                self.risk.current.overcopy_usdc = overcopy_usdc;
                self.risk.current.undercopy_usdc = undercopy_usdc;
                if let Some(trade) = self
                    .recent_trades
                    .iter_mut()
                    .rev()
                    .find(|trade| trade.asset == asset)
                {
                    trade.current_position_value_usdc = current_position_value_usdc;
                    trade.algo_target_risk_usdc = algo_target_risk_usdc;
                    trade.algo_delta_risk_usdc = algo_delta_risk_usdc;
                    trade.algo_reason = algo_reason.clone();
                }
            }
            MonEvent::ReconcileStart { leader } => {
                self.leader_mut(&leader).dirty = true;
            }
            MonEvent::ReconcileDone {
                leader,
                ok,
                latency_ms,
                positions,
                value_usdc,
                snapshot_age_ms,
                provisional_drift_bps,
            } => {
                let leader_mon = self.leader_mut(&leader);
                leader_mon.dirty = !ok;
                leader_mon
                    .reconcile_latency_ms
                    .record(now_ms, latency_ms as u64);
                leader_mon
                    .drift_bps
                    .record(now_ms, provisional_drift_bps as u64);
                leader_mon.snapshot_age_ms = snapshot_age_ms as u64;
                leader_mon.positions_count = positions;
                leader_mon.value_usdc = value_usdc;
                self.push_log(format!(
                    "leader {leader} reconcile {} positions={} value={value_usdc}",
                    if ok { "ok" } else { "error" },
                    positions
                ));
            }
            MonEvent::BookUpdate {
                asset,
                best_bid_ppm,
                best_ask_ppm,
                age_ms,
                levels_bid,
                levels_ask,
                crossed,
                hash_mismatch,
            } => {
                let asset_mon = self.asset_mut(&asset);
                asset_mon.age_ms.record(now_ms, age_ms as u64);
                asset_mon.last_age_ms = age_ms as u64;
                asset_mon.levels_bid = levels_bid;
                asset_mon.levels_ask = levels_ask;
                asset_mon.crossed = crossed;
                asset_mon.hash_mismatch = hash_mismatch;
                let bid = best_bid_ppm.max(1) as i64;
                let ask = best_ask_ppm.max(1) as i64;
                asset_mon.spread_bps = if ask > 0 {
                    (((ask - bid).max(0) as f64) / ask as f64 * 10_000.0).round() as u64
                } else {
                    0
                };
            }
            MonEvent::BookResync { asset, .. } => {
                self.asset_mut(&asset).resync_5m.incr(now_ms, 1);
                self.push_log(format!("book resync {asset}"));
            }
            MonEvent::SignalPlanned {
                asset,
                fresh_ms,
                agree_bps,
                raw_target_usdc,
                final_target_usdc,
                ..
            } => {
                self.signals.insert(
                    asset.clone(),
                    SignalMon {
                        status: "PLANNED".to_string(),
                        raw_target_usdc,
                        final_target_usdc,
                        agree_bps,
                        fresh_ms: fresh_ms as u64,
                        reason: None,
                    },
                );
                self.push_log(format!(
                    "signal planned {} raw={} final={} agree={}bp fresh={}ms",
                    asset, raw_target_usdc, final_target_usdc, agree_bps, fresh_ms
                ));
            }
            MonEvent::SignalSkipped {
                asset,
                reason,
                fresh_ms,
            } => {
                self.signals.insert(
                    asset.clone(),
                    SignalMon {
                        status: "SKIP".to_string(),
                        raw_target_usdc: 0,
                        final_target_usdc: 0,
                        agree_bps: 0,
                        fresh_ms: fresh_ms as u64,
                        reason: Some(reason.as_str().to_string()),
                    },
                );
                self.push_log(format!(
                    "signal skipped {} reason={} fresh={}ms",
                    asset,
                    reason.as_str(),
                    fresh_ms
                ));
            }
            MonEvent::PositionDiagnostics {
                target_count,
                delta_count,
                stale_asset_count,
                blocked_asset_count,
                blocker_summary,
            } => {
                self.position_targeting = PositionTargetingMon {
                    target_count,
                    delta_count,
                    stale_asset_count,
                    blocked_asset_count,
                    blocker_summary,
                };
                self.push_log(format!(
                    "position diagnostics targets={} deltas={} stale={} blocked={} blockers={}",
                    self.position_targeting.target_count,
                    self.position_targeting.delta_count,
                    self.position_targeting.stale_asset_count,
                    self.position_targeting.blocked_asset_count,
                    self.position_targeting.blocker_summary
                ));
            }
            MonEvent::OrderIntent { .. } => {
                self.exec.last_submit_status = "intent".to_string();
            }
            MonEvent::OrderPosted { latency_ms, .. } => {
                self.exec.last_submit_status = "posted".to_string();
                self.exec
                    .intent_to_post_ms
                    .record(now_ms, latency_ms as u64);
            }
            MonEvent::OrderMatched {
                order_id,
                matched_usdc,
                fee_usdc,
                copy_gap_bps,
                slip_bps,
                latency_ms,
                ..
            } => {
                self.exec.last_submit_status = "matched".to_string();
                self.exec.post_to_match_ms.record(now_ms, latency_ms as u64);
                self.exec.copy_gap_bps.record(now_ms, copy_gap_bps as u64);
                self.exec.slip_bps.record(now_ms, slip_bps as u64);
                let fee_adj = (copy_gap_bps as u64)
                    .saturating_add((fee_usdc.max(0) as u64) / matched_usdc.max(1) as u64);
                self.exec.fee_adj_slip_bps.record(now_ms, fee_adj);
                self.exec.fill_ratio_ppm.record(now_ms, 1_000_000);
                self.push_log(format!(
                    "order#{} matched usdc={} gap={}bp slip={}bp latency={}ms",
                    order_id, matched_usdc, copy_gap_bps, slip_bps, latency_ms
                ));
            }
            MonEvent::OrderConfirmed {
                order_id,
                latency_ms,
            } => {
                self.exec.last_submit_status = "confirmed".to_string();
                self.exec
                    .match_to_confirm_ms
                    .record(now_ms, latency_ms as u64);
                self.push_log(format!("order#{} confirmed {}ms", order_id, latency_ms));
            }
            MonEvent::OrderRejected { reason, .. } => {
                self.exec.last_submit_status = format!("rejected:{}", reason.as_str());
                self.push_log(format!("order rejected {}", reason.as_str()));
            }
            MonEvent::RiskSnapshot {
                equity_usdc,
                cash_usdc,
                deployed_usdc,
                gross_usdc,
                net_usdc,
                market_top1_usdc,
                event_top1_usdc,
                event_top3_usdc,
                tail_24h_usdc,
                tail_72h_usdc,
                neg_risk_usdc,
                tracking_err_bps,
                hhi_bps,
                follow_ratio_bps,
            } => {
                self.risk.current.equity_usdc = equity_usdc;
                self.risk.current.cash_usdc = cash_usdc;
                self.risk.current.deployed_usdc = deployed_usdc;
                self.risk.current.gross_usdc = gross_usdc;
                self.risk.current.net_usdc = net_usdc;
                self.risk.current.market_top1_usdc = market_top1_usdc;
                self.risk.current.event_top1_usdc = event_top1_usdc;
                self.risk.current.event_top3_usdc = event_top3_usdc;
                self.risk.current.tail_24h_usdc = tail_24h_usdc;
                self.risk.current.tail_72h_usdc = tail_72h_usdc;
                self.risk.current.neg_risk_usdc = neg_risk_usdc;
                self.risk.current.tracking_err_bps = tracking_err_bps;
                self.risk.current.hhi_bps = hhi_bps;
                self.risk.current.follow_ratio_bps = follow_ratio_bps;
                self.risk
                    .tracking_rmse
                    .record(now_ms, tracking_err_bps as u64);
            }
            MonEvent::AlertNote { level, msg } => {
                self.alerts.push(AlertView {
                    level,
                    key: "note".to_string(),
                    message: msg.clone(),
                });
                self.push_log(msg);
            }
            MonEvent::Tick { .. } | MonEvent::Shutdown => {}
        }
    }

    pub fn record_loop_lag(&mut self, now_ms: u64, lag_ms: u64) {
        self.proc.loop_lag_ms.record(now_ms, lag_ms);
    }

    pub fn set_proc_stats(
        &mut self,
        cpu_tenths_pct: u16,
        rss_mb: u64,
        open_fds: u64,
        threads: u64,
    ) {
        self.proc.cpu_tenths_pct = cpu_tenths_pct;
        self.proc.rss_mb = rss_mb;
        self.proc.open_fds = open_fds;
        self.proc.threads = threads;
    }

    pub fn set_build_label(&mut self, build_label: String) {
        self.proc.build_label = build_label;
    }

    pub fn build_snapshot(
        &mut self,
        now_ms: u64,
        monitor_dropped_total: u64,
        monitor_q_depth: u64,
        exec_q_depth: u64,
    ) -> UiSnapshot {
        let proc = ProcView {
            uptime_sec: now_ms.saturating_sub(self.proc.started_ms) / 1000,
            loop_lag_p95_ms: self.proc.loop_lag_ms.p95(now_ms),
            monitor_dropped_total,
            monitor_q_depth,
            exec_q_depth,
            cpu_tenths_pct: self.proc.cpu_tenths_pct,
            rss_mb: self.proc.rss_mb,
            open_fds: self.proc.open_fds,
            threads: self.proc.threads,
            build_label: self.proc.build_label.clone(),
        };

        let feeds = FeedView {
            market_ws: self.feeds.market_ws.view(now_ms),
            user_ws: self.feeds.user_ws.view(now_ms),
            data_api: self.feeds.data_api.view(now_ms),
            gamma_api: self.feeds.gamma_api.view(now_ms),
            clob_api: self.feeds.clob_api.view(now_ms),
        };

        let mut leaders = self
            .leaders
            .iter_mut()
            .map(|(leader, mon)| mon.view(now_ms, leader))
            .collect::<Vec<_>>();
        leaders.sort_by(|left, right| {
            right
                .value_usdc
                .cmp(&left.value_usdc)
                .then_with(|| left.leader.cmp(&right.leader))
        });
        leaders.truncate(self.cfg.top_k_leaders);

        let mut books = self
            .assets
            .iter_mut()
            .map(|(asset, mon)| mon.view(now_ms, asset))
            .collect::<Vec<_>>();
        books.sort_by(|left, right| {
            right
                .age_ms
                .cmp(&left.age_ms)
                .then_with(|| left.asset.cmp(&right.asset))
        });
        books.truncate(self.cfg.top_k_assets);

        let mut signals = self
            .signals
            .iter()
            .map(|(asset, mon)| SignalView {
                asset: asset.clone(),
                status: mon.status.clone(),
                raw_target_usdc: mon.raw_target_usdc,
                final_target_usdc: mon.final_target_usdc,
                agree_bps: mon.agree_bps,
                fresh_ms: mon.fresh_ms,
                reason: mon.reason.clone(),
            })
            .collect::<Vec<_>>();
        signals.sort_by(|left, right| left.asset.cmp(&right.asset));
        signals.truncate(self.cfg.top_k_assets);

        self.risk.current.rmse_1m_bps = self.risk.tracking_rmse.rmse(now_ms) as u16;
        let exec = self.exec.view(now_ms);
        let risk = self.risk.current.clone();
        let recent_logs = self.logs.iter().cloned().collect::<Vec<_>>();
        let ready = !self.selected_leader.wallet.is_empty()
            && leaders.iter().any(|leader| leader.positions_count > 0)
            && (!self.cfg.live_mode || self.feeds.user_ws.connected);

        let mut snapshot = UiSnapshot {
            now_ms: now_ms as i64,
            health: Health::Ok,
            mode: self.mode,
            ready,
            proc,
            feeds,
            selected_leader: SelectedLeaderView {
                wallet: self.selected_leader.wallet.clone(),
                source: self.selected_leader.source.clone(),
                category: self.selected_leader.category.clone(),
                score: self.selected_leader.score.clone(),
                review_status: self.selected_leader.review_status.clone(),
                core_pool: self.selected_leader.core_pool.clone(),
                active_pool: self.selected_leader.active_pool.clone(),
            },
            tracked_activity: TrackedActivityView {
                tx: self.tracked_activity.tx.clone(),
                side: self.tracked_activity.side.clone(),
                slug: self.tracked_activity.slug.clone(),
                asset: self.tracked_activity.asset.clone(),
                usdc_size: self.tracked_activity.usdc_size,
                price_ppm: self.tracked_activity.price_ppm,
                event_age_ms: self.tracked_activity.event_age_ms,
                event_ts_ms: self.tracked_activity.event_ts_ms,
                local_time_gmt8: self.tracked_activity.local_time_gmt8.clone(),
                current_position_value_usdc: self.tracked_activity.current_position_value_usdc,
                current_position_size: self.tracked_activity.current_position_size,
                current_avg_price_ppm: self.tracked_activity.current_avg_price_ppm,
                algo_target_risk_usdc: self.tracked_activity.algo_target_risk_usdc,
                algo_delta_risk_usdc: self.tracked_activity.algo_delta_risk_usdc,
                algo_confidence_bps: self.tracked_activity.algo_confidence_bps,
                algo_tte_bucket: self.tracked_activity.algo_tte_bucket.clone(),
                algo_reason: self.tracked_activity.algo_reason.clone(),
            },
            recent_trades: self
                .recent_trades
                .iter()
                .rev()
                .take(5)
                .map(|trade| TradeTapeView {
                    local_time_gmt8: trade.local_time_gmt8.clone(),
                    tx: trade.tx.clone(),
                    side: trade.side.clone(),
                    slug: trade.slug.clone(),
                    asset: trade.asset.clone(),
                    usdc_size: trade.usdc_size,
                    price_ppm: trade.price_ppm,
                    current_position_value_usdc: trade.current_position_value_usdc,
                    algo_target_risk_usdc: trade.algo_target_risk_usdc,
                    algo_delta_risk_usdc: trade.algo_delta_risk_usdc,
                    algo_reason: trade.algo_reason.clone(),
                })
                .collect(),
            leaders,
            books,
            signals,
            position_targeting: PositionTargetingView {
                target_count: self.position_targeting.target_count,
                delta_count: self.position_targeting.delta_count,
                stale_asset_count: self.position_targeting.stale_asset_count,
                blocked_asset_count: self.position_targeting.blocked_asset_count,
                blocker_summary: self.position_targeting.blocker_summary.clone(),
            },
            exec,
            risk,
            alerts: Vec::new(),
            recent_logs,
        };
        let (health, alerts) =
            super::alert::evaluate(&snapshot, &self.cfg.thresholds, self.cfg.live_mode);
        snapshot.health = health;
        snapshot.alerts = alerts;
        snapshot
    }
}

pub fn side_from_str(value: &str) -> Side {
    if value.eq_ignore_ascii_case("sell") {
        Side::Sell
    } else {
        Side::Buy
    }
}

pub fn skip_reason_from_text(value: &str) -> SkipReason {
    match value {
        "stale_signal" => SkipReason::StaleSignal,
        "stale_book" => SkipReason::StaleBook,
        "tail_window" => SkipReason::TailWindow,
        "neg_risk_blocked" => SkipReason::NegRiskBlocked,
        "gap_too_wide" => SkipReason::GapTooWide,
        "slip_too_wide" => SkipReason::SlipTooWide,
        "cash_cap" => SkipReason::CashCap,
        "risk_cap" => SkipReason::RiskCap,
        "user_ws_down" => SkipReason::UserWsDown,
        _ => SkipReason::NoLiquidity,
    }
}

pub fn reject_reason_from_status(value: &str) -> RejectReason {
    if value.contains("429") {
        RejectReason::RateLimited
    } else if value.contains("liquidity") {
        RejectReason::NoLiquidity
    } else if value.contains("gate") {
        RejectReason::GateBlocked
    } else {
        RejectReason::Unknown(value.to_string())
    }
}

fn format_gmt8(timestamp_ms: i64) -> String {
    if timestamp_ms <= 0 {
        return "none".to_string();
    }
    let total_secs = timestamp_ms.div_euclid(1000) + 8 * 60 * 60;
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let d = doy - (153 * mp + 2).div_euclid(5) + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} GMT+8",
        month = m,
        day = d
    )
}
