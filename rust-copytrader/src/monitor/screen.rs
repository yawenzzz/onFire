use super::snapshot::{Health, UiSnapshot};
use std::fmt::Write as _;

const ANSI_CLEAR: &str = "\x1b[2J\x1b[H";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_CYAN: &str = "\x1b[36m";

pub fn render(snapshot: &UiSnapshot) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}{}copytrader monitor v1{} {} {} HEALTH={}{}{}  equity={:.2} cash={:.2} deployed={:.2}",
        ANSI_CLEAR,
        ANSI_BOLD,
        ANSI_RESET,
        ANSI_CYAN,
        snapshot.mode,
        color_health(snapshot.health),
        snapshot.health,
        ANSI_RESET,
        usdc(snapshot.risk.equity_usdc),
        usdc(snapshot.risk.cash_usdc),
        usdc(snapshot.risk.deployed_usdc)
    );
    let _ = writeln!(
        out,
        "loop_lag_p95={}ms  mon_drop={}  q(mon)={}  q(exec)={}  ready={}",
        snapshot.proc.loop_lag_p95_ms,
        snapshot.proc.monitor_dropped_total,
        snapshot.proc.monitor_q_depth,
        snapshot.proc.exec_q_depth,
        snapshot.ready
    );
    let _ = writeln!(
        out,
        "rss={}MB  fds={}  threads={}",
        snapshot.proc.rss_mb, snapshot.proc.open_fds, snapshot.proc.threads
    );
    out.push('\n');

    section_header(&mut out, "feeds");
    let _ = writeln!(
        out,
        "  market_ws: {} age={}ms pong_p95={}ms reconnect={}",
        up(
            snapshot.feeds.market_ws.connected,
            snapshot.feeds.market_ws.note.as_deref()
        ),
        snapshot.feeds.market_ws.last_msg_age_ms,
        snapshot.feeds.market_ws.pong_p95_ms,
        snapshot.feeds.market_ws.reconnect_total,
    );
    let _ = writeln!(
        out,
        "  user_ws  : {} age={}ms pong_p95={}ms reconnect={}",
        up(
            snapshot.feeds.user_ws.connected,
            snapshot.feeds.user_ws.note.as_deref()
        ),
        snapshot.feeds.user_ws.last_msg_age_ms,
        snapshot.feeds.user_ws.pong_p95_ms,
        snapshot.feeds.user_ws.reconnect_total,
    );
    let _ = writeln!(
        out,
        "  data_api : p95={}ms  429_1m={}  5xx_1m={}  rl_fill={}%{}",
        snapshot.feeds.data_api.latency_p95_ms,
        snapshot.feeds.data_api.status_429_1m,
        snapshot.feeds.data_api.status_5xx_1m,
        snapshot.feeds.data_api.rl_fill_ratio_bps / 100,
        if snapshot.feeds.data_api.backoff_active {
            " backoff"
        } else {
            ""
        },
    );
    let _ = writeln!(
        out,
        "  gamma_api: p95={}ms  429_1m={}  5xx_1m={}  rl_fill={}%{}",
        snapshot.feeds.gamma_api.latency_p95_ms,
        snapshot.feeds.gamma_api.status_429_1m,
        snapshot.feeds.gamma_api.status_5xx_1m,
        snapshot.feeds.gamma_api.rl_fill_ratio_bps / 100,
        if snapshot.feeds.gamma_api.backoff_active {
            " backoff"
        } else {
            ""
        },
    );
    let _ = writeln!(
        out,
        "  clob_api : p95={}ms  429_1m={}  5xx_1m={}  rl_fill={}%{}",
        snapshot.feeds.clob_api.latency_p95_ms,
        snapshot.feeds.clob_api.status_429_1m,
        snapshot.feeds.clob_api.status_5xx_1m,
        snapshot.feeds.clob_api.rl_fill_ratio_bps / 100,
        if snapshot.feeds.clob_api.backoff_active {
            " backoff"
        } else {
            ""
        },
    );
    out.push('\n');

    section_header(&mut out, "selected leader");
    let _ = writeln!(
        out,
        "  wallet={} category={} score={} review={}",
        empty_as_none(&snapshot.selected_leader.wallet),
        empty_as_none(&snapshot.selected_leader.category),
        empty_as_none(&snapshot.selected_leader.score),
        empty_as_none(&snapshot.selected_leader.review_status),
    );
    let _ = writeln!(
        out,
        "  source={} core_pool={} active_pool={}",
        empty_as_none(&snapshot.selected_leader.source),
        empty_as_none(&snapshot.selected_leader.core_pool),
        empty_as_none(&snapshot.selected_leader.active_pool),
    );
    out.push('\n');

    section_header(&mut out, "tracked activity");
    let _ = writeln!(
        out,
        "  tx={} side={} slug={}",
        empty_as_none(&snapshot.tracked_activity.tx),
        empty_as_none(&snapshot.tracked_activity.side),
        empty_as_none(&snapshot.tracked_activity.slug),
    );
    let _ = writeln!(
        out,
        "  asset={} usdc={:.2} event_age={}ms event_ts={}",
        empty_as_none(&snapshot.tracked_activity.asset),
        usdc(snapshot.tracked_activity.usdc_size),
        snapshot.tracked_activity.event_age_ms,
        snapshot.tracked_activity.event_ts_ms,
    );
    out.push('\n');

    section_header(&mut out, "leaders");
    if snapshot.leaders.is_empty() {
        let _ = writeln!(out, "  none");
    } else {
        for leader in &snapshot.leaders {
            let _ = writeln!(
                out,
                "  {} activity_p95={}ms snap_age={}ms reconcile_p95={}ms drift_p95={}bp dirty={} positions={} value={:.2}",
                leader.leader,
                leader.activity_p95_ms,
                leader.snap_age_ms,
                leader.reconcile_p95_ms,
                leader.drift_p95_bps,
                if leader.dirty { "yes" } else { "no" },
                leader.positions_count,
                usdc(leader.value_usdc),
            );
            if let Some(slug) = &leader.last_slug {
                let _ = writeln!(
                    out,
                    "    last={} {} tx={}",
                    leader.last_side.as_deref().unwrap_or("-"),
                    slug,
                    leader.last_tx.as_deref().unwrap_or("none")
                );
            }
        }
    }
    out.push('\n');

    section_header(&mut out, "books");
    if snapshot.books.is_empty() {
        let _ = writeln!(out, "  none");
    } else {
        for book in &snapshot.books {
            let _ = writeln!(
                out,
                "  {} age={}ms spread={}bp levels={}/{} resync_5m={} crossed={} hash_mismatch={}",
                book.asset,
                book.age_ms,
                book.spread_bps,
                book.levels_bid,
                book.levels_ask,
                book.resync_5m,
                yn(book.crossed),
                yn(book.hash_mismatch),
            );
        }
    }
    out.push('\n');

    section_header(&mut out, "signals");
    if snapshot.signals.is_empty() {
        let _ = writeln!(out, "  none");
    } else {
        for signal in &snapshot.signals {
            if signal.status == "SKIP" {
                let _ = writeln!(
                    out,
                    "  {} SKIP {} fresh={}ms",
                    signal.asset,
                    signal.reason.as_deref().unwrap_or("unknown"),
                    signal.fresh_ms
                );
            } else {
                let _ = writeln!(
                    out,
                    "  {} raw={:+.2} final={:+.2} agree={}% fresh={}ms",
                    signal.asset,
                    usdc(signal.raw_target_usdc),
                    usdc(signal.final_target_usdc),
                    signal.agree_bps / 100,
                    signal.fresh_ms,
                );
            }
        }
    }
    out.push('\n');

    section_header(&mut out, "position targeting");
    let _ = writeln!(
        out,
        "  target_count={} delta_count={} stale_assets={} blocked_assets={}",
        snapshot.position_targeting.target_count,
        snapshot.position_targeting.delta_count,
        snapshot.position_targeting.stale_asset_count,
        snapshot.position_targeting.blocked_asset_count,
    );
    let _ = writeln!(
        out,
        "  blocker_summary={}",
        empty_as_none(&snapshot.position_targeting.blocker_summary),
    );
    out.push('\n');

    section_header(&mut out, "execution");
    let _ = writeln!(
        out,
        "  a->i p95={}ms  i->post p95={}ms  post->match p95={}ms  match->conf p95={}ms",
        snapshot.exec.activity_to_intent_p95_ms,
        snapshot.exec.intent_to_post_p95_ms,
        snapshot.exec.post_to_match_p95_ms,
        snapshot.exec.match_to_confirm_p95_ms,
    );
    let _ = writeln!(
        out,
        "  copy_gap p95={}bp  slip p95={}bp  fee_adj_slip p95={}bp  fill_ratio p50={}%  last_submit={}",
        snapshot.exec.copy_gap_p95_bps,
        snapshot.exec.slip_p95_bps,
        snapshot.exec.fee_adj_slip_p95_bps,
        snapshot.exec.fill_ratio_p50_ppm / 10_000,
        snapshot.exec.last_submit_status,
    );
    out.push('\n');

    section_header(&mut out, "risk");
    let _ = writeln!(
        out,
        "  gross={:.2} net={:.2} tail<24h={:.2} tail<72h={:.2} negRisk={:.2} HHI={}bp",
        usdc(snapshot.risk.gross_usdc),
        usdc(snapshot.risk.net_usdc),
        usdc(snapshot.risk.tail_24h_usdc),
        usdc(snapshot.risk.tail_72h_usdc),
        usdc(snapshot.risk.neg_risk_usdc),
        snapshot.risk.hhi_bps,
    );
    let _ = writeln!(
        out,
        "  track_err={}bp  rmse_1m={}bp  follow_ratio={}%",
        snapshot.risk.tracking_err_bps,
        snapshot.risk.rmse_1m_bps,
        snapshot.risk.follow_ratio_bps / 100,
    );
    out.push('\n');

    section_header(&mut out, "alerts");
    if snapshot.alerts.is_empty() {
        let _ = writeln!(out, "  none");
    } else {
        for alert in &snapshot.alerts {
            let _ = writeln!(out, "  {} {} {}", alert.level, alert.key, alert.message);
        }
    }
    out.push('\n');

    section_header(&mut out, "log");
    if snapshot.recent_logs.is_empty() {
        let _ = writeln!(out, "  none");
    } else {
        for line in snapshot.recent_logs.iter().rev().take(10).rev() {
            let _ = writeln!(out, "  {line}");
        }
    }
    out
}

pub fn render_health_json(snapshot: &UiSnapshot) -> String {
    let reasons = snapshot
        .alerts
        .iter()
        .map(|alert| format!("\"{}\"", escape_json(&alert.key)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"health\":\"{}\",\"reasons\":[{}],\"now_ms\":{}}}",
        snapshot.health, reasons, snapshot.now_ms
    )
}

pub fn render_metrics(snapshot: &UiSnapshot) -> String {
    format!(
        r#"# TYPE pm_proc_uptime_sec gauge
pm_proc_uptime_sec {}
# TYPE pm_proc_monitor_dropped_total counter
pm_proc_monitor_dropped_total {}
# TYPE pm_proc_main_loop_lag_p95_ms gauge
pm_proc_main_loop_lag_p95_ms {}
# TYPE pm_proc_rss_mb gauge
pm_proc_rss_mb {}
# TYPE pm_proc_open_fds gauge
pm_proc_open_fds {}
# TYPE pm_proc_threads gauge
pm_proc_threads {}
# TYPE pm_proc_monitor_queue_depth gauge
pm_proc_monitor_queue_depth {}
# TYPE pm_proc_strategy_queue_depth gauge
pm_proc_strategy_queue_depth {}
# TYPE pm_feed_ws_last_msg_age_ms gauge
pm_feed_ws_last_msg_age_ms{{channel="market"}} {}
pm_feed_ws_last_msg_age_ms{{channel="user"}} {}
# TYPE pm_feed_http_latency_p95_ms gauge
pm_feed_http_latency_p95_ms{{svc="data"}} {}
pm_feed_http_latency_p95_ms{{svc="gamma"}} {}
pm_feed_http_latency_p95_ms{{svc="clob"}} {}
# TYPE pm_leader_activity_event_age_p95_ms gauge
pm_leader_activity_event_age_p95_ms {}
# TYPE pm_leader_reconcile_latency_p95_ms gauge
pm_leader_reconcile_latency_p95_ms {}
# TYPE pm_book_age_p95_ms gauge
pm_book_age_p95_ms {}
# TYPE pm_leader_positions_count gauge
pm_leader_positions_count {}
# TYPE pm_exec_copy_gap_bps_p95 gauge
pm_exec_copy_gap_bps_p95 {}
# TYPE pm_exec_fee_adj_slip_bps_p95 gauge
pm_exec_fee_adj_slip_bps_p95 {}
# TYPE pm_track_error_rmse_bps_1m gauge
pm_track_error_rmse_bps_1m {}
# TYPE pm_risk_deployed_usdc gauge
pm_risk_deployed_usdc {}
# TYPE pm_risk_tail_lt24h_usdc gauge
pm_risk_tail_lt24h_usdc {}
# TYPE pm_risk_neg_risk_usdc gauge
pm_risk_neg_risk_usdc {}
# TYPE pm_risk_follow_ratio_bps gauge
pm_risk_follow_ratio_bps {}
# TYPE pm_position_targeting_target_count gauge
pm_position_targeting_target_count {}
# TYPE pm_position_targeting_delta_count gauge
pm_position_targeting_delta_count {}
# TYPE pm_position_targeting_blocked_asset_count gauge
pm_position_targeting_blocked_asset_count {}
"#,
        snapshot.proc.uptime_sec,
        snapshot.proc.monitor_dropped_total,
        snapshot.proc.loop_lag_p95_ms,
        snapshot.proc.rss_mb,
        snapshot.proc.open_fds,
        snapshot.proc.threads,
        snapshot.proc.monitor_q_depth,
        snapshot.proc.exec_q_depth,
        snapshot.feeds.market_ws.last_msg_age_ms,
        snapshot.feeds.user_ws.last_msg_age_ms,
        snapshot.feeds.data_api.latency_p95_ms,
        snapshot.feeds.gamma_api.latency_p95_ms,
        snapshot.feeds.clob_api.latency_p95_ms,
        snapshot
            .leaders
            .first()
            .map(|leader| leader.activity_p95_ms)
            .unwrap_or(0),
        snapshot
            .leaders
            .first()
            .map(|leader| leader.reconcile_p95_ms)
            .unwrap_or(0),
        snapshot.books.first().map(|book| book.age_ms).unwrap_or(0),
        snapshot
            .leaders
            .first()
            .map(|leader| leader.positions_count)
            .unwrap_or(0),
        snapshot.exec.copy_gap_p95_bps,
        snapshot.exec.fee_adj_slip_p95_bps,
        snapshot.risk.rmse_1m_bps,
        snapshot.risk.deployed_usdc,
        snapshot.risk.tail_24h_usdc,
        snapshot.risk.neg_risk_usdc,
        snapshot.risk.follow_ratio_bps,
        snapshot.position_targeting.target_count,
        snapshot.position_targeting.delta_count,
        snapshot.position_targeting.blocked_asset_count,
    )
}

fn section_header(out: &mut String, title: &str) {
    let _ = writeln!(out, "{}{}{}", ANSI_BOLD, title, ANSI_RESET);
}

fn color_health(health: Health) -> &'static str {
    match health {
        Health::Ok => ANSI_GREEN,
        Health::Warn => ANSI_YELLOW,
        Health::Crit => ANSI_RED,
    }
}

fn up(connected: bool, note: Option<&str>) -> String {
    if connected {
        "up".to_string()
    } else {
        note.unwrap_or("down").to_string()
    }
}

fn yn(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn usdc(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn empty_as_none(value: &str) -> &str {
    if value.is_empty() { "none" } else { value }
}
