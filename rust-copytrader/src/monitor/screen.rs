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
    render_for_width(snapshot, detect_width())
}

fn render_for_width(snapshot: &UiSnapshot, width: usize) -> String {
    if width < 110 {
        render_minimal(snapshot)
    } else if width < 140 {
        render_compact(snapshot)
    } else {
        render_standard(snapshot)
    }
}

fn render_standard(snapshot: &UiSnapshot) -> String {
    let mut out = String::new();
    let leader_rows = snapshot.leaders.iter().take(5).collect::<Vec<_>>();
    let book_rows = snapshot.books.iter().take(5).collect::<Vec<_>>();
    let signal_rows = snapshot.signals.iter().take(6).collect::<Vec<_>>();
    let recent_trade_rows = snapshot.recent_trades.iter().take(1).collect::<Vec<_>>();
    let mut alert_rows = snapshot.alerts.iter().collect::<Vec<_>>();
    alert_rows.sort_by(|left, right| {
        right
            .level
            .cmp(&left.level)
            .then_with(|| left.key.cmp(&right.key))
    });
    alert_rows.truncate(4);
    let log_rows = snapshot
        .recent_logs
        .iter()
        .rev()
        .take(6)
        .rev()
        .collect::<Vec<_>>();
    let mut rows: usize;
    let _ = writeln!(
        out,
        "{}{}{} {} HEALTH={}{}{}  eq={:.2} cash={:.2} dep={:.2} gross={:.2} net={:.2} up={}",
        ANSI_CLEAR,
        ANSI_BOLD,
        fmt_ts_gmt8(snapshot.now_ms),
        snapshot.mode,
        color_health(snapshot.health),
        snapshot.health,
        ANSI_RESET,
        usdc(snapshot.risk.equity_usdc),
        usdc(snapshot.risk.cash_usdc),
        usdc(snapshot.risk.deployed_usdc),
        usdc(snapshot.risk.gross_usdc),
        usdc(snapshot.risk.net_usdc),
        fmt_uptime(snapshot.proc.uptime_sec),
    );
    let _ = writeln!(
        out,
        "{}lag={}ms mon_drop={} q(mon)={} q(exec)={} rss={}MB fds={} threads={} ready={}{}",
        ANSI_CYAN,
        snapshot.proc.loop_lag_p95_ms,
        snapshot.proc.monitor_dropped_total,
        snapshot.proc.monitor_q_depth,
        snapshot.proc.exec_q_depth,
        snapshot.proc.rss_mb,
        snapshot.proc.open_fds,
        snapshot.proc.threads,
        snapshot.ready,
        ANSI_RESET,
    );
    out.push('\n');

    section_header(&mut out, "FEEDS");
    rows = 0;
    let _ = writeln!(
        out,
        "market_ws {} age={}ms pong_p95={}ms reconnect={} | user_ws {} age={}ms pong_p95={}ms reconnect={}",
        up(
            snapshot.feeds.market_ws.connected,
            snapshot.feeds.market_ws.note.as_deref()
        ),
        snapshot.feeds.market_ws.last_msg_age_ms,
        snapshot.feeds.market_ws.pong_p95_ms,
        snapshot.feeds.market_ws.reconnect_total,
        up(
            snapshot.feeds.user_ws.connected,
            snapshot.feeds.user_ws.note.as_deref()
        ),
        snapshot.feeds.user_ws.last_msg_age_ms,
        snapshot.feeds.user_ws.pong_p95_ms,
        snapshot.feeds.user_ws.reconnect_total,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "activity p95={}ms | gamma p95={}ms | clob p95={}ms | 429_1m d/g/c={}/{}/{}",
        snapshot.feeds.data_api.latency_p95_ms,
        snapshot.feeds.gamma_api.latency_p95_ms,
        snapshot.feeds.clob_api.latency_p95_ms,
        snapshot.feeds.data_api.status_429_1m,
        snapshot.feeds.gamma_api.status_429_1m,
        snapshot.feeds.clob_api.status_429_1m,
    );
    rows += 1;
    pad_rows(&mut out, rows, 2);
    out.push('\n');

    section_header(&mut out, "PROCESS");
    rows = 0;
    let _ = writeln!(
        out,
        "loop_p95={}ms mon_q={} exec_q={} dropped={} rss={}MB fds={} threads={}",
        snapshot.proc.loop_lag_p95_ms,
        snapshot.proc.monitor_q_depth,
        snapshot.proc.exec_q_depth,
        snapshot.proc.monitor_dropped_total,
        snapshot.proc.rss_mb,
        snapshot.proc.open_fds,
        snapshot.proc.threads,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "selected={} category={} score={} review={} source={}",
        empty_as_none(&snapshot.selected_leader.wallet),
        empty_as_none(&snapshot.selected_leader.category),
        empty_as_none(&snapshot.selected_leader.score),
        empty_as_none(&snapshot.selected_leader.review_status),
        empty_as_none(&snapshot.selected_leader.source),
    );
    rows += 1;
    pad_rows(&mut out, rows, 2);
    out.push('\n');

    section_header_count(
        &mut out,
        "TRADE TAPE",
        recent_trade_rows.len().max(1),
        snapshot.recent_trades.len().max(1),
    );
    rows = 0;
    let _ = writeln!(
        out,
        "latest tx={} side={} slug={}",
        empty_as_none(&snapshot.tracked_activity.tx),
        empty_as_none(&snapshot.tracked_activity.side),
        empty_as_none(&snapshot.tracked_activity.slug),
    );
    rows += 1;
    let _ = writeln!(
        out,
        "time={} asset={} usdc={:.2} px={:.4} age={}ms",
        empty_as_none(&snapshot.tracked_activity.local_time_gmt8),
        empty_as_none(&snapshot.tracked_activity.asset),
        usdc(snapshot.tracked_activity.usdc_size),
        ppm_price(snapshot.tracked_activity.price_ppm),
        snapshot.tracked_activity.event_age_ms,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "leader_pos={:.2} size={:.4} avg={:.4} algo_target={:.2} algo_delta={:.2}",
        usdc(snapshot.tracked_activity.current_position_value_usdc),
        shares(snapshot.tracked_activity.current_position_size),
        ppm_price(snapshot.tracked_activity.current_avg_price_ppm),
        usdc(snapshot.tracked_activity.algo_target_risk_usdc),
        usdc(snapshot.tracked_activity.algo_delta_risk_usdc),
    );
    rows += 1;
    let _ = writeln!(
        out,
        "conf={}bp tte={} reason={}",
        snapshot.tracked_activity.algo_confidence_bps,
        empty_as_none(&snapshot.tracked_activity.algo_tte_bucket),
        empty_as_none(&snapshot.tracked_activity.algo_reason),
    );
    rows += 1;
    if recent_trade_rows.is_empty() {
        let _ = writeln!(out, "recent=none");
        rows += 1;
    } else {
        for trade in recent_trade_rows {
            let _ = writeln!(
                out,
                "{} {} {} usdc={:.2} px={:.4} pos={:.2} tgt={:.2} Δ={:.2} {} tx={}",
                empty_as_none(&trade.local_time_gmt8),
                empty_as_none(&trade.side),
                empty_as_none(&trade.slug),
                usdc(trade.usdc_size),
                ppm_price(trade.price_ppm),
                usdc(trade.current_position_value_usdc),
                usdc(trade.algo_target_risk_usdc),
                usdc(trade.algo_delta_risk_usdc),
                empty_as_none(&trade.algo_reason),
                short_tx(&trade.tx),
            );
            rows += 1;
        }
    }
    pad_rows(&mut out, rows, 5);
    out.push('\n');

    section_header_count(
        &mut out,
        "LEADERS",
        leader_rows.len(),
        snapshot.leaders.len(),
    );
    rows = 0;
    if leader_rows.is_empty() {
        let _ = writeln!(out, "none");
        rows += 1;
    } else {
        for leader in leader_rows {
            let _ = writeln!(
                out,
                "{} stale={}ms drift={}bp dirty={} act_p95={}ms rec_p95={}ms pos={} val={:.2} last={} {}",
                elide(&leader.leader, 18),
                leader.snap_age_ms,
                leader.drift_p95_bps,
                if leader.dirty { "yes" } else { "no" },
                leader.activity_p95_ms,
                leader.reconcile_p95_ms,
                leader.positions_count,
                usdc(leader.value_usdc),
                leader.last_side.as_deref().unwrap_or("-"),
                elide(leader.last_slug.as_deref().unwrap_or("none"), 28),
            );
            rows += 1;
        }
    }
    pad_rows(&mut out, rows, 5);
    out.push('\n');

    section_header_count(
        &mut out,
        "HOT ASSETS",
        book_rows.len(),
        snapshot.books.len(),
    );
    rows = 0;
    if book_rows.is_empty() {
        let _ = writeln!(out, "none");
        rows += 1;
    } else {
        for book in book_rows {
            let _ = writeln!(
                out,
                "{} age={}ms spread={}bp levels={}/{} crossed={} resync={} hash_mismatch={}",
                book.asset,
                book.age_ms,
                book.spread_bps,
                book.levels_bid,
                book.levels_ask,
                book.resync_5m,
                yn(book.crossed),
                yn(book.hash_mismatch),
            );
            rows += 1;
        }
    }
    pad_rows(&mut out, rows, 5);
    out.push('\n');

    section_header_count(
        &mut out,
        "SIGNALS",
        signal_rows.len(),
        snapshot.signals.len(),
    );
    rows = 0;
    if signal_rows.is_empty() {
        let _ = writeln!(out, "none");
        rows += 1;
    } else {
        for signal in signal_rows {
            if signal.status == "SKIP" {
                let _ = writeln!(
                    out,
                    "{} SKIP {} fresh={}ms",
                    signal.asset,
                    signal.reason.as_deref().unwrap_or("unknown"),
                    signal.fresh_ms
                );
                rows += 1;
            } else {
                let _ = writeln!(
                    out,
                    "{} raw={:+.2} final={:+.2} agree={}% fresh={}ms",
                    signal.asset,
                    usdc(signal.raw_target_usdc),
                    usdc(signal.final_target_usdc),
                    signal.agree_bps / 100,
                    signal.fresh_ms,
                );
                rows += 1;
            }
        }
    }
    pad_rows(&mut out, rows, 6);
    out.push('\n');

    section_header(&mut out, "EXECUTION");
    rows = 0;
    let _ = writeln!(
        out,
        "a->i {}ms  i->post {}ms  post->match {}ms  conf {}ms",
        snapshot.exec.activity_to_intent_p95_ms,
        snapshot.exec.intent_to_post_p95_ms,
        snapshot.exec.post_to_match_p95_ms,
        snapshot.exec.match_to_confirm_p95_ms,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "gap {}bp  slip {}bp  fee_adj {}bp  fill {}%  last={}",
        snapshot.exec.copy_gap_p95_bps,
        snapshot.exec.slip_p95_bps,
        snapshot.exec.fee_adj_slip_p95_bps,
        snapshot.exec.fill_ratio_p50_ppm / 10_000,
        snapshot.exec.last_submit_status,
    );
    rows += 1;
    pad_rows(&mut out, rows, 2);
    out.push('\n');

    section_header(&mut out, "RISK");
    rows = 0;
    let _ = writeln!(
        out,
        "market_top1={:.2} event_top1={:.2} event_top3={:.2}",
        usdc(snapshot.risk.market_top1_usdc),
        usdc(snapshot.risk.event_top1_usdc),
        usdc(snapshot.risk.event_top3_usdc),
    );
    rows += 1;
    let _ = writeln!(
        out,
        "tail<24h={:.2} tail<72h={:.2} negRisk={:.2} hhi={}bp",
        usdc(snapshot.risk.tail_24h_usdc),
        usdc(snapshot.risk.tail_72h_usdc),
        usdc(snapshot.risk.neg_risk_usdc),
        snapshot.risk.hhi_bps,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "target_count={} delta_count={} stale_assets={} blocked_assets={}",
        snapshot.position_targeting.target_count,
        snapshot.position_targeting.delta_count,
        snapshot.position_targeting.stale_asset_count,
        snapshot.position_targeting.blocked_asset_count,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "blocker_summary={}",
        empty_as_none(&snapshot.position_targeting.blocker_summary),
    );
    rows += 1;
    pad_rows(&mut out, rows, 4);
    out.push('\n');

    section_header(&mut out, "TRACKING");
    rows = 0;
    let _ = writeln!(
        out,
        "track_err={}bp rmse_1m={}bp follow_ratio={}%",
        snapshot.risk.tracking_err_bps,
        snapshot.risk.rmse_1m_bps,
        snapshot.risk.follow_ratio_bps / 100,
    );
    rows += 1;
    let _ = writeln!(
        out,
        "eligible={:.2} copied={:.2} overcopy={:.2} undercopy={:.2}",
        usdc(snapshot.risk.deployed_usdc),
        usdc(snapshot.risk.deployed_usdc * snapshot.risk.follow_ratio_bps as i64 / 10_000),
        0.0,
        usdc(snapshot.risk.deployed_usdc)
            - usdc(snapshot.risk.deployed_usdc * snapshot.risk.follow_ratio_bps as i64 / 10_000),
    );
    rows += 1;
    pad_rows(&mut out, rows, 2);
    out.push('\n');

    section_header_count(&mut out, "ALERTS", alert_rows.len(), snapshot.alerts.len());
    rows = 0;
    if alert_rows.is_empty() {
        let _ = writeln!(out, "none");
        rows += 1;
    } else {
        for alert in alert_rows {
            let _ = writeln!(out, "{} {} {}", alert.level, alert.key, alert.message);
            rows += 1;
        }
    }
    pad_rows(&mut out, rows, 4);
    out.push('\n');

    section_header_count(&mut out, "LOGS", log_rows.len(), snapshot.recent_logs.len());
    rows = 0;
    if log_rows.is_empty() {
        let _ = writeln!(out, "none");
        rows += 1;
    } else {
        for line in log_rows {
            let _ = writeln!(out, "{line}");
            rows += 1;
        }
    }
    pad_rows(&mut out, rows, 6);
    out
}

fn render_compact(snapshot: &UiSnapshot) -> String {
    let mut out = String::new();
    let mut alert_rows = snapshot.alerts.iter().collect::<Vec<_>>();
    alert_rows.sort_by(|left, right| {
        right
            .level
            .cmp(&left.level)
            .then_with(|| left.key.cmp(&right.key))
    });
    let _ = writeln!(
        out,
        "{}{}{} {} {} eq={:.2} cash={:.2} dep={:.2} gross={:.2} net={:.2} up={}",
        ANSI_CLEAR,
        ANSI_BOLD,
        fmt_ts_gmt8(snapshot.now_ms),
        snapshot.mode,
        color_health_label(snapshot.health),
        usdc(snapshot.risk.equity_usdc),
        usdc(snapshot.risk.cash_usdc),
        usdc(snapshot.risk.deployed_usdc),
        usdc(snapshot.risk.gross_usdc),
        usdc(snapshot.risk.net_usdc),
        fmt_uptime(snapshot.proc.uptime_sec),
    );
    let _ = writeln!(
        out,
        "{}lag={}ms mon_drop={} q={}/{} rss={}MB ready={}{}",
        ANSI_CYAN,
        snapshot.proc.loop_lag_p95_ms,
        snapshot.proc.monitor_dropped_total,
        snapshot.proc.monitor_q_depth,
        snapshot.proc.exec_q_depth,
        snapshot.proc.rss_mb,
        snapshot.ready,
        ANSI_RESET,
    );
    out.push('\n');
    section_header(&mut out, "FEEDS");
    let _ = writeln!(
        out,
        "mkt_ws {} {}ms | user_ws {} {}ms | activity p95 {}ms | positions p95 {}ms | books age {}ms",
        up(
            snapshot.feeds.market_ws.connected,
            snapshot.feeds.market_ws.note.as_deref()
        ),
        snapshot.feeds.market_ws.last_msg_age_ms,
        up(
            snapshot.feeds.user_ws.connected,
            snapshot.feeds.user_ws.note.as_deref()
        ),
        snapshot.feeds.user_ws.last_msg_age_ms,
        snapshot.feeds.data_api.latency_p95_ms,
        snapshot
            .leaders
            .first()
            .map(|l| l.reconcile_p95_ms)
            .unwrap_or(0),
        snapshot.books.first().map(|b| b.age_ms).unwrap_or(0),
    );
    section_header(&mut out, "TRADE");
    let _ = writeln!(
        out,
        "{} {} {} usdc={:.2} px={:.4} pos={:.2} tgt={:.2} Δ={:.2} {}",
        empty_as_none(&snapshot.tracked_activity.local_time_gmt8),
        empty_as_none(&snapshot.tracked_activity.side),
        empty_as_none(&snapshot.tracked_activity.slug),
        usdc(snapshot.tracked_activity.usdc_size),
        ppm_price(snapshot.tracked_activity.price_ppm),
        usdc(snapshot.tracked_activity.current_position_value_usdc),
        usdc(snapshot.tracked_activity.algo_target_risk_usdc),
        usdc(snapshot.tracked_activity.algo_delta_risk_usdc),
        empty_as_none(&snapshot.tracked_activity.algo_reason),
    );
    section_header_count(
        &mut out,
        "LEADERS",
        snapshot.leaders.iter().take(2).count(),
        snapshot.leaders.len(),
    );
    for leader in snapshot.leaders.iter().take(2) {
        let _ = writeln!(
            out,
            "{} stale={}ms drift={}bp pos={} val={:.2}",
            leader.leader,
            leader.snap_age_ms,
            leader.drift_p95_bps,
            leader.positions_count,
            usdc(leader.value_usdc),
        );
    }
    section_header_count(
        &mut out,
        "HOT ASSETS",
        snapshot.books.iter().take(3).count(),
        snapshot.books.len(),
    );
    let _ = writeln!(
        out,
        "{}",
        snapshot
            .books
            .iter()
            .take(3)
            .map(|book| format!(
                "{} age={}ms spread={}bp",
                book.asset, book.age_ms, book.spread_bps
            ))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    section_header_count(
        &mut out,
        "SIGNALS",
        snapshot.signals.iter().take(3).count(),
        snapshot.signals.len(),
    );
    for signal in snapshot.signals.iter().take(3) {
        if signal.status == "SKIP" {
            let _ = writeln!(
                out,
                "{} SKIP {} fresh={}ms",
                signal.asset,
                signal.reason.as_deref().unwrap_or("unknown"),
                signal.fresh_ms
            );
        } else {
            let _ = writeln!(
                out,
                "{} raw={:+.2} final={:+.2} fresh={}ms",
                signal.asset,
                usdc(signal.raw_target_usdc),
                usdc(signal.final_target_usdc),
                signal.fresh_ms
            );
        }
    }
    section_header(&mut out, "EXEC / RISK");
    let _ = writeln!(
        out,
        "a->i {}ms i->post {}ms post->match {}ms conf {}ms",
        snapshot.exec.activity_to_intent_p95_ms,
        snapshot.exec.intent_to_post_p95_ms,
        snapshot.exec.post_to_match_p95_ms,
        snapshot.exec.match_to_confirm_p95_ms,
    );
    let _ = writeln!(
        out,
        "gap {}bp slip {}bp fee_adj {}bp fill {}%",
        snapshot.exec.copy_gap_p95_bps,
        snapshot.exec.slip_p95_bps,
        snapshot.exec.fee_adj_slip_p95_bps,
        snapshot.exec.fill_ratio_p50_ppm / 10_000
    );
    let _ = writeln!(
        out,
        "tail24={:.2} tail72={:.2} neg={:.2} hhi={} follow={} rmse={}bp",
        usdc(snapshot.risk.tail_24h_usdc),
        usdc(snapshot.risk.tail_72h_usdc),
        usdc(snapshot.risk.neg_risk_usdc),
        snapshot.risk.hhi_bps,
        snapshot.risk.follow_ratio_bps / 100,
        snapshot.risk.rmse_1m_bps
    );
    section_header_count(
        &mut out,
        "ALERTS",
        alert_rows.iter().take(3).count(),
        snapshot.alerts.len(),
    );
    for alert in alert_rows.iter().take(3) {
        let _ = writeln!(out, "{} {} {}", alert.level, alert.key, alert.message);
    }
    section_header_count(
        &mut out,
        "LOGS",
        snapshot.recent_logs.iter().rev().take(4).count(),
        snapshot.recent_logs.len(),
    );
    for line in snapshot.recent_logs.iter().rev().take(4).rev() {
        let _ = writeln!(out, "{line}");
    }
    out
}

fn render_minimal(snapshot: &UiSnapshot) -> String {
    let mut out = String::new();
    let mut alert_rows = snapshot.alerts.iter().collect::<Vec<_>>();
    alert_rows.sort_by(|left, right| {
        right
            .level
            .cmp(&left.level)
            .then_with(|| left.key.cmp(&right.key))
    });
    let _ = writeln!(
        out,
        "{}{}{} {} {} eq={:.2} dep={:.2}{}",
        ANSI_CLEAR,
        ANSI_BOLD,
        fmt_ts_gmt8(snapshot.now_ms),
        snapshot.mode,
        color_health_label(snapshot.health),
        usdc(snapshot.risk.equity_usdc),
        usdc(snapshot.risk.deployed_usdc),
        ANSI_RESET,
    );
    let _ = writeln!(
        out,
        "feeds m={}ms u={}ms act={}ms rec={}ms",
        snapshot.feeds.market_ws.last_msg_age_ms,
        snapshot.feeds.user_ws.last_msg_age_ms,
        snapshot.feeds.data_api.latency_p95_ms,
        snapshot
            .leaders
            .first()
            .map(|l| l.reconcile_p95_ms)
            .unwrap_or(0),
    );
    let _ = writeln!(
        out,
        "trade {} {} usdc={:.2} px={:.4}",
        empty_as_none(&snapshot.tracked_activity.side),
        empty_as_none(&snapshot.tracked_activity.slug),
        usdc(snapshot.tracked_activity.usdc_size),
        ppm_price(snapshot.tracked_activity.price_ppm),
    );
    let _ = writeln!(
        out,
        "exec gap={}bp slip={}bp fill={}%",
        snapshot.exec.copy_gap_p95_bps,
        snapshot.exec.slip_p95_bps,
        snapshot.exec.fill_ratio_p50_ppm / 10_000
    );
    let _ = writeln!(
        out,
        "risk tail24={:.2} neg={:.2} rmse={}bp",
        usdc(snapshot.risk.tail_24h_usdc),
        usdc(snapshot.risk.neg_risk_usdc),
        snapshot.risk.rmse_1m_bps
    );
    let _ = writeln!(
        out,
        "alerts {}",
        alert_rows
            .iter()
            .take(2)
            .map(|alert| format!("{}:{}", alert.level, alert.key))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    out
}

fn detect_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(140)
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

fn section_header_count(out: &mut String, title: &str, shown: usize, total: usize) {
    let _ = writeln!(
        out,
        "{}{} [{} / {}]{}",
        ANSI_BOLD, title, shown, total, ANSI_RESET
    );
}

fn pad_rows(out: &mut String, rendered: usize, min_rows: usize) {
    for _ in rendered..min_rows {
        let _ = writeln!(out);
    }
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

fn color_health_label(health: Health) -> String {
    format!("{}{}{}", color_health(health), health, ANSI_RESET)
}

fn fmt_uptime(uptime_sec: u64) -> String {
    let hours = uptime_sec / 3_600;
    let minutes = (uptime_sec % 3_600) / 60;
    let seconds = uptime_sec % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn fmt_ts_gmt8(timestamp_ms: i64) -> String {
    if timestamp_ms <= 0 {
        return "0000-00-00 00:00:00 GMT+8".to_string();
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

fn yn(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn usdc(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

fn shares(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

fn ppm_price(value: i32) -> f64 {
    value as f64 / 1_000_000.0
}

fn short_tx(value: &str) -> String {
    if value.len() <= 18 {
        value.to_string()
    } else {
        format!("{}…{}", &value[..10], &value[value.len() - 6..])
    }
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

fn elide(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        let prefix = value.chars().take(max_chars - 1).collect::<String>();
        format!("{prefix}…")
    }
}

#[cfg(test)]
mod tests {
    use super::{UiSnapshot, render_for_width};
    use crate::monitor::snapshot::{
        AlertView, BookViewUi, ExecView, FeedChannelView, FeedHttpView, FeedView, Health,
        LeaderView, PositionTargetingView, ProcView, RiskView, SelectedLeaderView, SignalView,
        TrackedActivityView, TradeTapeView,
    };

    fn sample_snapshot() -> UiSnapshot {
        UiSnapshot {
            now_ms: 1_776_444_836_000,
            health: Health::Warn,
            proc: ProcView {
                uptime_sec: 3661,
                loop_lag_p95_ms: 42,
                monitor_dropped_total: 0,
                monitor_q_depth: 2,
                exec_q_depth: 1,
                rss_mb: 12,
                open_fds: 6,
                threads: 3,
            },
            feeds: FeedView {
                market_ws: FeedChannelView {
                    connected: true,
                    last_msg_age_ms: 120,
                    pong_p95_ms: 210,
                    reconnect_total: 1,
                    decode_err_total: 0,
                    note: None,
                },
                user_ws: FeedChannelView {
                    connected: true,
                    last_msg_age_ms: 480,
                    pong_p95_ms: 260,
                    reconnect_total: 0,
                    decode_err_total: 0,
                    note: None,
                },
                data_api: FeedHttpView {
                    latency_p95_ms: 280,
                    status_429_1m: 0,
                    status_5xx_1m: 0,
                    rl_fill_ratio_bps: 7300,
                    backoff_active: false,
                },
                gamma_api: FeedHttpView::default(),
                clob_api: FeedHttpView {
                    latency_p95_ms: 160,
                    status_429_1m: 0,
                    status_5xx_1m: 0,
                    rl_fill_ratio_bps: 6800,
                    backoff_active: false,
                },
            },
            selected_leader: SelectedLeaderView {
                wallet: "0xleader".into(),
                source: "summary".into(),
                category: "TECH".into(),
                score: "84".into(),
                review_status: "stable".into(),
                core_pool: "none".into(),
                active_pool: "none".into(),
            },
            tracked_activity: TrackedActivityView {
                tx: "0xtx".into(),
                side: "BUY".into(),
                slug: "market-a".into(),
                asset: "asset-1".into(),
                usdc_size: 57_000_000,
                price_ppm: 420000,
                event_age_ms: 1200,
                event_ts_ms: 1_776_444_836_000,
                local_time_gmt8: "2026-04-18 12:53:56 GMT+8".into(),
                current_position_value_usdc: 95_000_000,
                current_position_size: 100_000_000,
                current_avg_price_ppm: 410000,
                algo_target_risk_usdc: 57_000_000,
                algo_delta_risk_usdc: 12_000_000,
                algo_confidence_bps: 8200,
                algo_tte_bucket: "Over72h".into(),
                algo_reason: "plannable".into(),
            },
            recent_trades: vec![TradeTapeView {
                local_time_gmt8: "2026-04-18 12:53:56 GMT+8".into(),
                tx: "0xtx".into(),
                side: "BUY".into(),
                slug: "market-a".into(),
                asset: "asset-1".into(),
                usdc_size: 57_000_000,
                price_ppm: 420000,
                current_position_value_usdc: 95_000_000,
                algo_target_risk_usdc: 57_000_000,
                algo_delta_risk_usdc: 12_000_000,
                algo_reason: "plannable".into(),
            }],
            leaders: vec![LeaderView {
                leader: "alice".into(),
                activity_p95_ms: 820,
                snap_age_ms: 800,
                reconcile_p95_ms: 220,
                drift_p95_bps: 48,
                dirty: false,
                positions_count: 7,
                value_usdc: 182_340_000_000,
                last_tx: Some("0xtx".into()),
                last_side: Some("BUY".into()),
                last_slug: Some("market-a".into()),
            }],
            books: vec![BookViewUi {
                asset: "market-a".into(),
                age_ms: 90,
                spread_bps: 18,
                levels_bid: 18,
                levels_ask: 22,
                resync_5m: 0,
                crossed: false,
                hash_mismatch: false,
            }],
            signals: vec![SignalView {
                asset: "market-a".into(),
                status: "PLANNED".into(),
                raw_target_usdc: 152_000_000,
                final_target_usdc: 57_000_000,
                agree_bps: 8200,
                fresh_ms: 1200,
                reason: None,
            }],
            position_targeting: PositionTargetingView {
                target_count: 1,
                delta_count: 1,
                stale_asset_count: 0,
                blocked_asset_count: 0,
                blocker_summary: "none".into(),
            },
            exec: ExecView {
                activity_to_intent_p95_ms: 190,
                intent_to_post_p95_ms: 82,
                post_to_match_p95_ms: 640,
                match_to_confirm_p95_ms: 1800,
                copy_gap_p95_bps: 24,
                slip_p95_bps: 18,
                fee_adj_slip_p95_bps: 31,
                fill_ratio_p50_ppm: 1_000_000,
                last_submit_status: "confirmed".into(),
            },
            risk: RiskView {
                equity_usdc: 12_430_550_000,
                cash_usdc: 9_328_340_000,
                deployed_usdc: 3_102_210_000,
                gross_usdc: 3_880_100_000,
                net_usdc: 1_844_200_000,
                market_top1_usdc: 152_000_000,
                event_top1_usdc: 214_000_000,
                event_top3_usdc: 522_000_000,
                tail_24h_usdc: 0,
                tail_72h_usdc: 214_000_000,
                neg_risk_usdc: 0,
                tracking_err_bps: 47,
                rmse_1m_bps: 63,
                follow_ratio_bps: 8400,
                hhi_bps: 1380,
            },
            alerts: vec![AlertView {
                level: Health::Warn,
                key: "positions_slow".into(),
                message: "bob reconcile p95 910ms".into(),
            }],
            recent_logs: vec!["14:32:08 MATCHED trump-yes buy 57.00 gap=21bp slip=18bp".into()],
            ..UiSnapshot::default()
        }
    }

    #[test]
    fn render_standard_v2_layout_contains_major_sections() {
        let rendered = render_for_width(&sample_snapshot(), 140);
        assert!(rendered.contains("FEEDS"));
        assert!(rendered.contains("PROCESS"));
        assert!(rendered.contains("TRADE TAPE"));
        assert!(rendered.contains("LEADERS"));
        assert!(rendered.contains("HOT ASSETS"));
        assert!(rendered.contains("SIGNALS"));
        assert!(rendered.contains("EXECUTION"));
        assert!(rendered.contains("RISK"));
        assert!(rendered.contains("TRACKING"));
    }

    #[test]
    fn render_compact_layout_contains_key_sections() {
        let rendered = render_for_width(&sample_snapshot(), 110);
        assert!(rendered.contains("FEEDS"));
        assert!(rendered.contains("TRADE"));
        assert!(rendered.contains("EXEC / RISK"));
        assert!(rendered.contains("ALERTS"));
    }
}
