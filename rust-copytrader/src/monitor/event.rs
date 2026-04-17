use super::snapshot::Health;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Svc {
    Gamma,
    Data,
    Clob,
    Local,
}

impl Svc {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gamma => "gamma",
            Self::Data => "data",
            Self::Clob => "clob",
            Self::Local => "local",
        }
    }
}

impl fmt::Display for Svc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsCh {
    Market,
    User,
}

impl WsCh {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    StaleSignal,
    StaleBook,
    TailWindow,
    NegRiskBlocked,
    GapTooWide,
    SlipTooWide,
    NoLiquidity,
    RiskCap,
    CashCap,
    UserWsDown,
}

impl SkipReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleSignal => "stale_signal",
            Self::StaleBook => "stale_book",
            Self::TailWindow => "tail_window",
            Self::NegRiskBlocked => "neg_risk_blocked",
            Self::GapTooWide => "gap_too_wide",
            Self::SlipTooWide => "slip_too_wide",
            Self::NoLiquidity => "no_liquidity",
            Self::RiskCap => "risk_cap",
            Self::CashCap => "cash_cap",
            Self::UserWsDown => "user_ws_down",
        }
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    GateBlocked,
    RateLimited,
    NoLiquidity,
    Unknown(String),
}

impl RejectReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::GateBlocked => "gate_blocked",
            Self::RateLimited => "rate_limited",
            Self::NoLiquidity => "no_liquidity",
            Self::Unknown(reason) => reason.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonEvent {
    HttpDone {
        svc: Svc,
        route: String,
        status: u16,
        latency_ms: u32,
        bytes: u32,
    },
    WsConnected {
        ch: WsCh,
    },
    WsDisconnected {
        ch: WsCh,
        reason: String,
    },
    WsPong {
        ch: WsCh,
        rtt_ms: u32,
    },
    WsMsg {
        ch: WsCh,
        kind: String,
        recv_ts_ms: i64,
    },
    LeaderSelected {
        wallet: String,
        source: String,
        category: String,
        score: String,
        review_status: String,
        core_pool: String,
        active_pool: String,
    },
    ActivityHit {
        leader: String,
        asset: String,
        condition: String,
        side: Side,
        usdc_size: i64,
        leader_price_ppm: i32,
        event_ts_ms: i64,
        recv_ts_ms: i64,
        tx_hash: String,
        slug: Option<String>,
    },
    ReconcileStart {
        leader: String,
    },
    ReconcileDone {
        leader: String,
        ok: bool,
        latency_ms: u32,
        positions: u16,
        value_usdc: i64,
        snapshot_age_ms: u32,
        provisional_drift_bps: u16,
    },
    BookUpdate {
        asset: String,
        best_bid_ppm: i32,
        best_ask_ppm: i32,
        age_ms: u32,
        levels_bid: u16,
        levels_ask: u16,
        crossed: bool,
        hash_mismatch: bool,
    },
    BookResync {
        asset: String,
        age_ms: u32,
    },
    SignalPlanned {
        asset: String,
        leaders: u8,
        fresh_ms: u32,
        agree_bps: u16,
        raw_target_usdc: i64,
        final_target_usdc: i64,
    },
    SignalSkipped {
        asset: String,
        reason: SkipReason,
        fresh_ms: u32,
    },
    PositionDiagnostics {
        target_count: u64,
        delta_count: u64,
        stale_asset_count: u64,
        blocked_asset_count: u64,
        blocker_summary: String,
    },
    OrderIntent {
        order_id: u64,
        asset: String,
        side: Side,
        target_usdc: i64,
        leader_px_ppm: i32,
        bbo_px_ppm: i32,
    },
    OrderPosted {
        order_id: u64,
        latency_ms: u32,
    },
    OrderMatched {
        order_id: u64,
        matched_usdc: i64,
        matched_shares: i64,
        eff_px_ppm: i32,
        fee_usdc: i64,
        copy_gap_bps: u16,
        slip_bps: u16,
        latency_ms: u32,
    },
    OrderConfirmed {
        order_id: u64,
        latency_ms: u32,
    },
    OrderRejected {
        order_id: u64,
        reason: RejectReason,
    },
    RiskSnapshot {
        equity_usdc: i64,
        cash_usdc: i64,
        deployed_usdc: i64,
        gross_usdc: i64,
        net_usdc: i64,
        tail_24h_usdc: i64,
        tail_72h_usdc: i64,
        neg_risk_usdc: i64,
        tracking_err_bps: u16,
        hhi_bps: u16,
        follow_ratio_bps: u16,
    },
    AlertNote {
        level: Health,
        msg: String,
    },
    Tick {
        now_ms: i64,
    },
    Shutdown,
}
