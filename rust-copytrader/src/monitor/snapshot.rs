use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Health {
    #[default]
    Ok,
    Warn,
    Crit,
}

impl Health {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warn => "WARN",
            Self::Crit => "CRIT",
        }
    }

    pub const fn http_status(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Warn => 429,
            Self::Crit => 503,
        }
    }
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Live,
    #[default]
    ShadowPoll,
    Replay,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::ShadowPoll => "SHADOW",
            Self::Replay => "REPLAY",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcView {
    pub uptime_sec: u64,
    pub loop_lag_p95_ms: u64,
    pub monitor_dropped_total: u64,
    pub monitor_q_depth: u64,
    pub exec_q_depth: u64,
    pub rss_mb: u64,
    pub open_fds: u64,
    pub threads: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedChannelView {
    pub connected: bool,
    pub last_msg_age_ms: u64,
    pub pong_p95_ms: u64,
    pub reconnect_total: u64,
    pub decode_err_total: u64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedHttpView {
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
    pub status_429_1m: u64,
    pub status_5xx_1m: u64,
    pub rl_fill_ratio_bps: u16,
    pub backoff_active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedView {
    pub market_ws: FeedChannelView,
    pub user_ws: FeedChannelView,
    pub data_api: FeedHttpView,
    pub gamma_api: FeedHttpView,
    pub clob_api: FeedHttpView,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedLeaderView {
    pub wallet: String,
    pub source: String,
    pub category: String,
    pub score: String,
    pub review_status: String,
    pub core_pool: String,
    pub active_pool: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackedActivityView {
    pub tx: String,
    pub side: String,
    pub slug: String,
    pub asset: String,
    pub usdc_size: i64,
    pub price_ppm: i32,
    pub event_age_ms: u64,
    pub event_ts_ms: i64,
    pub local_time_gmt8: String,
    pub current_position_value_usdc: i64,
    pub current_position_size: i64,
    pub current_avg_price_ppm: i32,
    pub algo_target_risk_usdc: i64,
    pub algo_delta_risk_usdc: i64,
    pub algo_confidence_bps: u16,
    pub algo_tte_bucket: String,
    pub algo_reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TradeTapeView {
    pub local_time_gmt8: String,
    pub tx: String,
    pub side: String,
    pub slug: String,
    pub asset: String,
    pub usdc_size: i64,
    pub price_ppm: i32,
    pub current_position_value_usdc: i64,
    pub algo_target_risk_usdc: i64,
    pub algo_delta_risk_usdc: i64,
    pub algo_reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaderView {
    pub leader: String,
    pub activity_p95_ms: u64,
    pub snap_age_ms: u64,
    pub reconcile_p95_ms: u64,
    pub drift_p95_bps: u64,
    pub dirty: bool,
    pub positions_count: u16,
    pub value_usdc: i64,
    pub last_tx: Option<String>,
    pub last_side: Option<String>,
    pub last_slug: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookViewUi {
    pub asset: String,
    pub age_ms: u64,
    pub spread_bps: u64,
    pub levels_bid: u16,
    pub levels_ask: u16,
    pub resync_5m: u64,
    pub crossed: bool,
    pub hash_mismatch: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignalView {
    pub asset: String,
    pub status: String,
    pub raw_target_usdc: i64,
    pub final_target_usdc: i64,
    pub agree_bps: u16,
    pub fresh_ms: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PositionTargetingView {
    pub target_count: u64,
    pub delta_count: u64,
    pub stale_asset_count: u64,
    pub blocked_asset_count: u64,
    pub blocker_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecView {
    pub activity_to_intent_p95_ms: u64,
    pub intent_to_post_p95_ms: u64,
    pub post_to_match_p95_ms: u64,
    pub match_to_confirm_p95_ms: u64,
    pub copy_gap_p95_bps: u64,
    pub slip_p95_bps: u64,
    pub fee_adj_slip_p95_bps: u64,
    pub fill_ratio_p50_ppm: u64,
    pub last_submit_status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiskView {
    pub equity_usdc: i64,
    pub cash_usdc: i64,
    pub deployed_usdc: i64,
    pub gross_usdc: i64,
    pub net_usdc: i64,
    pub market_top1_usdc: i64,
    pub event_top1_usdc: i64,
    pub event_top3_usdc: i64,
    pub tail_24h_usdc: i64,
    pub tail_72h_usdc: i64,
    pub neg_risk_usdc: i64,
    pub tracking_err_bps: u16,
    pub rmse_1m_bps: u16,
    pub follow_ratio_bps: u16,
    pub eligible_usdc: i64,
    pub copied_usdc: i64,
    pub overcopy_usdc: i64,
    pub undercopy_usdc: i64,
    pub hhi_bps: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlertView {
    pub level: Health,
    pub key: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiSnapshot {
    pub now_ms: i64,
    pub health: Health,
    pub mode: Mode,
    pub ready: bool,
    pub proc: ProcView,
    pub feeds: FeedView,
    pub selected_leader: SelectedLeaderView,
    pub tracked_activity: TrackedActivityView,
    pub recent_trades: Vec<TradeTapeView>,
    pub leaders: Vec<LeaderView>,
    pub books: Vec<BookViewUi>,
    pub signals: Vec<SignalView>,
    pub position_targeting: PositionTargetingView,
    pub exec: ExecView,
    pub risk: RiskView,
    pub alerts: Vec<AlertView>,
    pub recent_logs: Vec<String>,
}
