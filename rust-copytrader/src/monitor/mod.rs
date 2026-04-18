pub mod alert;
pub mod event;
pub mod hist;
pub mod http;
pub mod journal;
pub mod rolling;
pub mod screen;
pub mod snapshot;
pub mod state;

use self::event::MonEvent;
use self::journal::{MonitorJournal, ensure_parent, escape_json};
use self::screen::{render, render_health_json, render_metrics};
use self::snapshot::{Mode, UiSnapshot};
use self::state::MonState;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorThresholds {
    pub main_loop_lag_warn_ms: u64,
    pub main_loop_lag_crit_ms: u64,
    pub activity_event_age_warn_ms: u64,
    pub activity_event_age_crit_ms: u64,
    pub market_ws_age_warn_ms: u64,
    pub market_ws_age_crit_ms: u64,
    pub user_ws_age_warn_ms: u64,
    pub user_ws_age_crit_ms: u64,
    pub http_429_warn_ratio_bps: u64,
    pub http_429_crit_ratio_bps: u64,
    pub reconcile_warn_ms: u64,
    pub reconcile_crit_ms: u64,
    pub book_age_warn_ms: u64,
    pub book_age_crit_ms: u64,
    pub copy_gap_warn_bps: u64,
    pub copy_gap_crit_bps: u64,
    pub track_rmse_warn_bps: u64,
    pub track_rmse_crit_bps: u64,
    pub monitor_drop_crit_per_min: u64,
    pub stale_assets_warn_ratio_bps: u64,
    pub stale_assets_crit_ratio_bps: u64,
}

impl Default for MonitorThresholds {
    fn default() -> Self {
        Self {
            main_loop_lag_warn_ms: 100,
            main_loop_lag_crit_ms: 300,
            activity_event_age_warn_ms: 3_000,
            activity_event_age_crit_ms: 10_000,
            market_ws_age_warn_ms: 3_000,
            market_ws_age_crit_ms: 10_000,
            user_ws_age_warn_ms: 5_000,
            user_ws_age_crit_ms: 15_000,
            http_429_warn_ratio_bps: 100,
            http_429_crit_ratio_bps: 500,
            reconcile_warn_ms: 1_200,
            reconcile_crit_ms: 3_000,
            book_age_warn_ms: 1_000,
            book_age_crit_ms: 3_000,
            copy_gap_warn_bps: 60,
            copy_gap_crit_bps: 100,
            track_rmse_warn_bps: 120,
            track_rmse_crit_bps: 250,
            monitor_drop_crit_per_min: 100,
            stale_assets_warn_ratio_bps: 1_000,
            stale_assets_crit_ratio_bps: 2_500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorCfg {
    pub snapshot_dir: PathBuf,
    pub journal_dir: PathBuf,
    pub ui_refresh_ms: u64,
    pub top_k_assets: usize,
    pub top_k_leaders: usize,
    pub log_lines: usize,
    pub journal_rotate_mb: u64,
    pub journal_keep_files: usize,
    pub queue_capacity: usize,
    pub live_mode: bool,
    pub http_bind: Option<String>,
    pub thresholds: MonitorThresholds,
}

impl Default for MonitorCfg {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from(".omx/monitor"),
            journal_dir: PathBuf::from(".omx/monitor/journal"),
            ui_refresh_ms: 500,
            top_k_assets: 12,
            top_k_leaders: 8,
            log_lines: 200,
            journal_rotate_mb: 64,
            journal_keep_files: 5,
            queue_capacity: 2_048,
            live_mode: false,
            http_bind: None,
            thresholds: MonitorThresholds::default(),
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle {
    tx: SyncSender<MonEvent>,
    dropped: Arc<AtomicU64>,
    queue_depth: Arc<AtomicU64>,
    snapshot: Arc<RwLock<UiSnapshot>>,
}

impl MonitorHandle {
    pub fn emit(&self, ev: MonEvent) {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
        match self.tx.try_send(ev) {
            Ok(()) => {}
            Err(_) => {
                self.queue_depth
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                        Some(value.saturating_sub(1))
                    })
                    .ok();
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn snapshot(&self) -> Arc<RwLock<UiSnapshot>> {
        Arc::clone(&self.snapshot)
    }

    pub fn dropped_total(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn queue_depth(&self) -> u64 {
        self.queue_depth.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.emit(MonEvent::Shutdown);
    }
}

pub struct MonitorRuntime {
    pub handle: MonitorHandle,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    http_join: Option<JoinHandle<()>>,
}

impl MonitorRuntime {
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.shutdown();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        if let Some(join) = self.http_join.take() {
            let _ = join.join();
        }
    }
}

pub fn spawn_monitor(cfg: MonitorCfg, mode: Mode) -> io::Result<MonitorRuntime> {
    let (tx, rx) = mpsc::sync_channel(cfg.queue_capacity.max(1));
    let dropped = Arc::new(AtomicU64::new(0));
    let queue_depth = Arc::new(AtomicU64::new(0));
    let snapshot = Arc::new(RwLock::new(UiSnapshot::default()));
    let stop = Arc::new(AtomicBool::new(false));

    let handle = MonitorHandle {
        tx,
        dropped: Arc::clone(&dropped),
        queue_depth: Arc::clone(&queue_depth),
        snapshot: Arc::clone(&snapshot),
    };

    let cfg_for_thread = cfg.clone();
    let stop_for_thread = Arc::clone(&stop);
    let snapshot_for_thread = Arc::clone(&snapshot);
    let join = thread::spawn(move || {
        let mut state = MonState::new(cfg_for_thread.clone(), mode);
        let mut journal = MonitorJournal::new(
            cfg_for_thread.journal_dir.clone(),
            cfg_for_thread.journal_rotate_mb.saturating_mul(1024 * 1024),
            cfg_for_thread.journal_keep_files,
        )
        .ok();
        let mut last_ui_ms = now_ms_u64();
        let mut last_proc_sample_ms = 0u64;

        while !stop_for_thread.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(MonEvent::Shutdown) => {
                    queue_depth
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                            Some(value.saturating_sub(1))
                        })
                        .ok();
                    break;
                }
                Ok(event) => {
                    queue_depth
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                            Some(value.saturating_sub(1))
                        })
                        .ok();
                    if let Some(journal) = journal.as_mut()
                        && let Some(line) = journal_line(now_ms_u64(), &event)
                    {
                        let _ = journal.append(&line);
                    }
                    state.apply(event);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            let now_ms = now_ms_u64();
            if now_ms.saturating_sub(last_ui_ms) >= cfg_for_thread.ui_refresh_ms.max(100) {
                let lag_ms =
                    now_ms.saturating_sub(last_ui_ms.saturating_add(cfg_for_thread.ui_refresh_ms));
                state.record_loop_lag(now_ms, lag_ms);
                if now_ms.saturating_sub(last_proc_sample_ms) >= 5_000 {
                    if let Some((rss_mb, open_fds, threads)) = sample_process_stats() {
                        state.set_proc_stats(rss_mb, open_fds, threads);
                    }
                    last_proc_sample_ms = now_ms;
                }
                let snapshot_value = state.build_snapshot(
                    now_ms,
                    dropped.load(Ordering::Relaxed),
                    queue_depth.load(Ordering::Relaxed),
                    0,
                );
                if let Ok(mut guard) = snapshot_for_thread.write() {
                    *guard = snapshot_value.clone();
                }
                let _ = persist_snapshot_artifacts(&cfg_for_thread, &snapshot_value);
                last_ui_ms = now_ms;
            }
        }
    });

    let http_join = if let Some(bind) = cfg.http_bind.clone() {
        Some(http::spawn_http_server(
            bind,
            Arc::clone(&snapshot),
            Arc::clone(&stop),
            cfg.live_mode,
        )?)
    } else {
        None
    };

    Ok(MonitorRuntime {
        handle,
        stop,
        join: Some(join),
        http_join,
    })
}

fn persist_snapshot_artifacts(cfg: &MonitorCfg, snapshot: &UiSnapshot) -> io::Result<()> {
    let latest_screen = cfg.snapshot_dir.join("latest.txt");
    let latest_metrics = cfg.snapshot_dir.join("metrics.txt");
    let latest_health = cfg.snapshot_dir.join("health.json");
    ensure_parent(&latest_screen)?;
    std::fs::write(&latest_screen, render(snapshot))?;
    std::fs::write(&latest_metrics, render_metrics(snapshot))?;
    std::fs::write(&latest_health, render_health_json(snapshot))?;
    Ok(())
}

fn journal_line(now_ms: u64, event: &MonEvent) -> Option<String> {
    match event {
        MonEvent::LeaderSelected {
            wallet,
            category,
            score,
            review_status,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"leader_selected\",\"wallet\":\"{}\",\"category\":\"{}\",\"score\":\"{}\",\"review\":\"{}\"}}",
            now_ms,
            escape_json(wallet),
            escape_json(category),
            escape_json(score),
            escape_json(review_status),
        )),
        MonEvent::ActivityHit {
            leader,
            asset,
            tx_hash,
            side,
            slug,
            usdc_size,
            leader_price_ppm,
            event_ts_ms,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"activity_hit\",\"leader\":\"{}\",\"asset\":\"{}\",\"side\":\"{}\",\"slug\":{},\"tx\":\"{}\",\"usdc_size\":{},\"leader_price_ppm\":{},\"event_ts_ms\":{}}}",
            now_ms,
            escape_json(leader),
            escape_json(asset),
            side.as_str(),
            slug.as_ref()
                .map(|value| format!("\"{}\"", escape_json(value)))
                .unwrap_or_else(|| "null".to_string()),
            escape_json(tx_hash),
            usdc_size,
            leader_price_ppm,
            event_ts_ms,
        )),
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
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"tracked_activity_projection\",\"asset\":\"{}\",\"current_position_value_usdc\":{},\"current_position_size\":{},\"current_avg_price_ppm\":{},\"algo_target_risk_usdc\":{},\"algo_delta_risk_usdc\":{},\"algo_confidence_bps\":{},\"algo_tte_bucket\":\"{}\",\"algo_reason\":\"{}\",\"tracking_err_bps\":{},\"follow_ratio_bps\":{},\"copied_usdc\":{},\"overcopy_usdc\":{},\"undercopy_usdc\":{}}}",
            now_ms,
            escape_json(asset),
            current_position_value_usdc,
            current_position_size,
            current_avg_price_ppm,
            algo_target_risk_usdc,
            algo_delta_risk_usdc,
            algo_confidence_bps,
            escape_json(algo_tte_bucket),
            escape_json(algo_reason),
            tracking_err_bps,
            follow_ratio_bps,
            copied_usdc,
            overcopy_usdc,
            undercopy_usdc,
        )),
        MonEvent::ReconcileDone {
            leader,
            ok,
            latency_ms,
            positions,
            value_usdc,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"reconcile_done\",\"leader\":\"{}\",\"ok\":{},\"latency_ms\":{},\"positions\":{},\"value_usdc\":{}}}",
            now_ms,
            escape_json(leader),
            ok,
            latency_ms,
            positions,
            value_usdc,
        )),
        MonEvent::PositionDiagnostics {
            target_count,
            delta_count,
            stale_asset_count,
            blocked_asset_count,
            blocker_summary,
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"position_diagnostics\",\"target_count\":{},\"delta_count\":{},\"stale_asset_count\":{},\"blocked_asset_count\":{},\"blocker_summary\":\"{}\"}}",
            now_ms,
            target_count,
            delta_count,
            stale_asset_count,
            blocked_asset_count,
            escape_json(blocker_summary),
        )),
        MonEvent::SignalPlanned {
            asset,
            fresh_ms,
            agree_bps,
            raw_target_usdc,
            final_target_usdc,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"signal_planned\",\"asset\":\"{}\",\"fresh_ms\":{},\"agree_bps\":{},\"raw_target_usdc\":{},\"final_target_usdc\":{}}}",
            now_ms,
            escape_json(asset),
            fresh_ms,
            agree_bps,
            raw_target_usdc,
            final_target_usdc,
        )),
        MonEvent::SignalSkipped {
            asset,
            reason,
            fresh_ms,
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"signal_skipped\",\"asset\":\"{}\",\"reason\":\"{}\",\"fresh_ms\":{}}}",
            now_ms,
            escape_json(asset),
            reason.as_str(),
            fresh_ms,
        )),
        MonEvent::BookResync { asset, age_ms } => Some(format!(
            "{{\"ts\":{},\"k\":\"book_resync\",\"asset\":\"{}\",\"age_ms\":{}}}",
            now_ms,
            escape_json(asset),
            age_ms,
        )),
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
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"risk_snapshot\",\"equity_usdc\":{},\"cash_usdc\":{},\"deployed_usdc\":{},\"gross_usdc\":{},\"net_usdc\":{},\"market_top1_usdc\":{},\"event_top1_usdc\":{},\"event_top3_usdc\":{},\"tail_24h_usdc\":{},\"tail_72h_usdc\":{},\"neg_risk_usdc\":{},\"tracking_err_bps\":{},\"hhi_bps\":{},\"follow_ratio_bps\":{}}}",
            now_ms,
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
        )),
        MonEvent::OrderPosted {
            order_id,
            latency_ms,
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"order_posted\",\"order_id\":{},\"latency_ms\":{}}}",
            now_ms, order_id, latency_ms
        )),
        MonEvent::OrderMatched {
            order_id,
            copy_gap_bps,
            slip_bps,
            fee_usdc,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"order_matched\",\"order_id\":{},\"copy_gap_bps\":{},\"slip_bps\":{},\"fee_usdc\":{}}}",
            now_ms, order_id, copy_gap_bps, slip_bps, fee_usdc
        )),
        MonEvent::OrderConfirmed {
            order_id,
            latency_ms,
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"order_confirmed\",\"order_id\":{},\"latency_ms\":{}}}",
            now_ms, order_id, latency_ms
        )),
        MonEvent::OrderRejected { order_id, reason } => Some(format!(
            "{{\"ts\":{},\"k\":\"order_rejected\",\"order_id\":{},\"reason\":\"{}\"}}",
            now_ms,
            order_id,
            escape_json(reason.as_str())
        )),
        MonEvent::AlertNote { level, msg } => Some(format!(
            "{{\"ts\":{},\"k\":\"alert\",\"level\":\"{}\",\"msg\":\"{}\"}}",
            now_ms,
            level,
            escape_json(msg)
        )),
        MonEvent::HttpDone {
            svc,
            route,
            status,
            latency_ms,
            ..
        } => Some(format!(
            "{{\"ts\":{},\"k\":\"http_done\",\"svc\":\"{}\",\"route\":\"{}\",\"status\":{},\"latency_ms\":{}}}",
            now_ms,
            svc.as_str(),
            escape_json(route),
            status,
            latency_ms,
        )),
        MonEvent::Tick { .. }
        | MonEvent::Shutdown
        | MonEvent::WsConnected { .. }
        | MonEvent::WsDisconnected { .. }
        | MonEvent::WsPong { .. }
        | MonEvent::WsMsg { .. }
        | MonEvent::BookUpdate { .. }
        | MonEvent::OrderIntent { .. }
        | MonEvent::ReconcileStart { .. } => None,
    }
}

pub fn now_ms_u64() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sample_process_stats() -> Option<(u64, u64, u64)> {
    let pid = std::process::id().to_string();
    let rss_kb = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.split_whitespace().next()?.parse::<u64>().ok())
        .unwrap_or(0);
    let threads = Command::new("ps")
        .args(["-M", "-p", &pid])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.lines().skip(1).count().max(1) as u64)
        .unwrap_or(0);
    let open_fds = std::fs::read_dir("/proc/self/fd")
        .or_else(|_| std::fs::read_dir("/dev/fd"))
        .ok()
        .map(|entries| entries.filter_map(Result::ok).count() as u64)
        .unwrap_or(0);
    Some((rss_kb / 1024, open_fds, threads))
}
