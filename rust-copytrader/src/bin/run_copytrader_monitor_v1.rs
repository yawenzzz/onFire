use rust_copytrader::adapters::positions::parse_leader_positions_payload;
use rust_copytrader::monitor::event::SkipReason;
use rust_copytrader::monitor::event::{MonEvent, Svc};
use rust_copytrader::monitor::screen;
use rust_copytrader::monitor::snapshot::{Health, Mode};
use rust_copytrader::monitor::state::{reject_reason_from_status, side_from_str};
use rust_copytrader::monitor::{MonitorCfg, MonitorRuntime, now_ms_u64, spawn_monitor};
use rust_copytrader::wallet_filter::{
    parse_activity_records, parse_iso8601_timestamp, parse_position_records, parse_total_value,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    summary_path: Option<String>,
    selected_leader_env: Option<String>,
    proxy: Option<String>,
    poll_interval_ms: u64,
    reconcile_interval_ms: u64,
    ui_refresh_ms: u64,
    iterations: Option<usize>,
    once: bool,
    auto_select: bool,
    http_enabled: bool,
    http_bind: String,
    select_bin: Option<String>,
    watch_bin: Option<String>,
    position_targeting_bin: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            summary_path: None,
            selected_leader_env: None,
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            poll_interval_ms: 5_000,
            reconcile_interval_ms: 10_000,
            ui_refresh_ms: 500,
            iterations: None,
            once: false,
            auto_select: true,
            http_enabled: true,
            http_bind: "127.0.0.1:9911".to_string(),
            select_bin: None,
            watch_bin: None,
            position_targeting_bin: None,
        }
    }
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            return ExitCode::from(2);
        }
    };

    match run_monitor(&options) {
        Ok(frame) => {
            if options.once {
                print!("{frame}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!(
        "usage: run_copytrader_monitor_v1 [--root <path>] [--summary <path>] [--selected-leader-env <path>] [--proxy <url>] [--poll-interval-ms <n>] [--reconcile-interval-ms <n>] [--ui-refresh-ms <n>] [--iterations <n>] [--once] [--no-http] [--http-bind <addr>] [--no-auto-select] [--select-bin <path>] [--watch-bin <path>] [--position-targeting-bin <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--summary" => options.summary_path = Some(next_value(&mut iter, arg)?),
            "--selected-leader-env" => {
                options.selected_leader_env = Some(next_value(&mut iter, arg)?)
            }
            "--proxy" => options.proxy = Some(next_value(&mut iter, arg)?),
            "--poll-interval-ms" => {
                options.poll_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "poll-interval-ms")?
            }
            "--reconcile-interval-ms" => {
                options.reconcile_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "reconcile-interval-ms")?
            }
            "--ui-refresh-ms" => {
                options.ui_refresh_ms = parse_u64(&next_value(&mut iter, arg)?, "ui-refresh-ms")?
            }
            "--iterations" => {
                options.iterations = Some(parse_usize(&next_value(&mut iter, arg)?, "iterations")?)
            }
            "--once" => {
                options.once = true;
                options.iterations = Some(1);
            }
            "--no-auto-select" => options.auto_select = false,
            "--no-http" => options.http_enabled = false,
            "--http-bind" => options.http_bind = next_value(&mut iter, arg)?,
            "--select-bin" => options.select_bin = Some(next_value(&mut iter, arg)?),
            "--watch-bin" => options.watch_bin = Some(next_value(&mut iter, arg)?),
            "--position-targeting-bin" => {
                options.position_targeting_bin = Some(next_value(&mut iter, arg)?)
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(options)
}

fn next_value<'a, I>(iter: &mut I, flag: &str) -> Result<String, String>
where
    I: Iterator<Item = &'a String>,
{
    iter.next()
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn run_monitor(options: &Options) -> Result<String, String> {
    let root = PathBuf::from(&options.root);
    let monitor_dir = root.join(".omx/monitor");
    let cfg = MonitorCfg {
        snapshot_dir: monitor_dir.clone(),
        journal_dir: monitor_dir.join("journal"),
        ui_refresh_ms: options.ui_refresh_ms,
        http_bind: options.http_enabled.then(|| options.http_bind.clone()),
        ..MonitorCfg::default()
    };
    let runtime = spawn_monitor(cfg, Mode::ShadowPoll)
        .map_err(|error| format!("failed to start monitor: {error}"))?;
    runtime.handle.emit(MonEvent::AlertNote {
        level: Health::Ok,
        msg: "monitor v1 started in shadow poll mode".to_string(),
    });
    runtime.handle.emit(MonEvent::WsDisconnected {
        ch: rust_copytrader::monitor::event::WsCh::Market,
        reason: "shadow_poll".to_string(),
    });
    runtime.handle.emit(MonEvent::WsDisconnected {
        ch: rust_copytrader::monitor::event::WsCh::User,
        reason: "shadow_poll".to_string(),
    });

    let render_stop = Arc::new(AtomicBool::new(false));
    let render_join = if options.once {
        None
    } else {
        let snapshot = runtime.handle.snapshot();
        let render_stop = Arc::clone(&render_stop);
        let interval_ms = options.ui_refresh_ms.max(100);
        Some(thread::spawn(move || {
            while !render_stop.load(Ordering::Relaxed) {
                if let Ok(guard) = snapshot.read() {
                    print!("{}", screen::render(&guard));
                }
                thread::sleep(Duration::from_millis(interval_ms));
            }
        }))
    };

    let select_bin = resolve_bin_path("select_copy_leader", options.select_bin.as_deref())
        .map_err(|error| format!("failed to resolve select_copy_leader: {error}"))?;
    let watch_bin = resolve_bin_path("watch_copy_leader_activity", options.watch_bin.as_deref())
        .map_err(|error| format!("failed to resolve watch_copy_leader_activity: {error}"))?;
    let position_bin = resolve_bin_path(
        "run_position_targeting_demo",
        options.position_targeting_bin.as_deref(),
    )
    .map_err(|error| format!("failed to resolve run_position_targeting_demo: {error}"))?;

    let summary_path = options
        .summary_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery/wallet-filter-v1-summary.txt"));
    let selected_env = options
        .selected_leader_env
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery/selected-leader.env"));

    if options.auto_select || !selected_env.exists() {
        select_leader(&select_bin, &summary_path, &selected_env)?;
    }

    emit_selected_leader(&runtime, &selected_env)?;

    let mut last_seen_tx = None::<String>;
    let mut active_wallet = None::<String>;
    let mut last_operator_signature = None::<String>;
    let mut last_reconcile_ms = 0u64;
    let iterations = options.iterations.unwrap_or(usize::MAX);
    for index in 0..iterations {
        if options.auto_select {
            select_leader(&select_bin, &summary_path, &selected_env)?;
        }
        emit_selected_leader(&runtime, &selected_env)?;
        let wallet = load_selected_wallet(&selected_env)?;
        if active_wallet.as_deref() != Some(wallet.as_str()) {
            last_seen_tx = None;
            active_wallet = Some(wallet.clone());
            runtime.handle.emit(MonEvent::AlertNote {
                level: Health::Ok,
                msg: format!("selected leader switched to {wallet}"),
            });
        }
        let watch_output = run_watch_cycle(
            &runtime,
            &watch_bin,
            &root,
            &wallet,
            options.proxy.as_deref(),
            &mut last_seen_tx,
        )?;
        if let Some(note) = watch_output.get("latest_new_slug") {
            runtime.handle.emit(MonEvent::AlertNote {
                level: Health::Ok,
                msg: format!("tracked {wallet} latest slug {note}"),
            });
        }

        let now_ms = now_ms_u64();
        if index == 0 || now_ms.saturating_sub(last_reconcile_ms) >= options.reconcile_interval_ms {
            run_position_cycle(
                &runtime,
                &position_bin,
                &root,
                &wallet,
                watch_output.get("latest_new_asset").map(String::as_str),
            )?;
            emit_operator_exec_metrics(&runtime, &root, &mut last_operator_signature)?;
            last_reconcile_ms = now_ms;
        }

        runtime.handle.emit(MonEvent::Tick {
            now_ms: now_ms as i64,
        });
        if options.once || index + 1 >= iterations {
            break;
        }
        thread::sleep(Duration::from_millis(options.poll_interval_ms.max(100)));
    }

    let drain_deadline =
        std::time::Instant::now() + Duration::from_millis(options.ui_refresh_ms.max(100) * 4);
    while runtime.handle.queue_depth() > 0 && std::time::Instant::now() < drain_deadline {
        thread::sleep(Duration::from_millis(25));
    }
    thread::sleep(Duration::from_millis(options.ui_refresh_ms.max(100)));
    let frame = runtime
        .handle
        .snapshot()
        .read()
        .map(|guard| screen::render(&guard))
        .unwrap_or_default();
    render_stop.store(true, Ordering::Relaxed);
    if let Some(join) = render_join {
        let _ = join.join();
    }
    runtime.shutdown();
    Ok(frame)
}

fn select_leader(select_bin: &Path, summary_path: &Path, output_path: &Path) -> Result<(), String> {
    let args = vec![
        "--summary".to_string(),
        summary_path.display().to_string(),
        "--output".to_string(),
        output_path.display().to_string(),
    ];
    run_command(select_bin, &args, Some(Path::new("."))).map(|_| ())
}

fn emit_selected_leader(runtime: &MonitorRuntime, selected_env: &Path) -> Result<(), String> {
    let values = parse_key_values(
        &fs::read_to_string(selected_env)
            .map_err(|error| format!("failed to read {}: {error}", selected_env.display()))?,
    );
    runtime.handle.emit(MonEvent::LeaderSelected {
        wallet: values
            .get("COPYTRADER_DISCOVERY_WALLET")
            .or_else(|| values.get("COPYTRADER_LEADER_WALLET"))
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        source: values
            .get("COPYTRADER_SELECTED_FROM")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        category: values
            .get("COPYTRADER_SELECTED_CATEGORY")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        score: values
            .get("COPYTRADER_SELECTED_SCORE")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        review_status: values
            .get("COPYTRADER_SELECTED_REVIEW_STATUS")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        core_pool: values
            .get("COPYTRADER_CORE_POOL_WALLETS")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        active_pool: values
            .get("COPYTRADER_ACTIVE_POOL_WALLETS")
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
    });
    Ok(())
}

fn load_selected_wallet(path: &Path) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    content
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            match key.trim() {
                "COPYTRADER_DISCOVERY_WALLET" | "COPYTRADER_LEADER_WALLET" => {
                    Some(value.trim().to_string())
                }
                _ => None,
            }
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing wallet in {}", path.display()))
}

fn run_watch_cycle(
    runtime: &MonitorRuntime,
    watch_bin: &Path,
    root: &Path,
    wallet: &str,
    proxy: Option<&str>,
    last_seen_tx: &mut Option<String>,
) -> Result<BTreeMap<String, String>, String> {
    let args = build_watch_args(root, wallet, proxy);
    let started = std::time::Instant::now();
    let output = run_command(watch_bin, &args, Some(Path::new(".")))?;
    let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    runtime.handle.emit(MonEvent::HttpDone {
        svc: Svc::Data,
        route: "activity".to_string(),
        status: 200,
        latency_ms: elapsed_ms,
        bytes: output.stdout.len().min(u32::MAX as usize) as u32,
    });
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("watch stdout was not utf-8: {error}"))?;
    let mut values = parse_key_values(&stdout);
    let latest_path = values
        .get("watch_latest_path")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(".omx/live-activity")
                .join(wallet)
                .join("latest-activity.json")
        });
    if latest_path.exists() {
        let body = fs::read_to_string(&latest_path)
            .map_err(|error| format!("failed to read {}: {error}", latest_path.display()))?;
        let events = parse_activity_records(&body);
        if let Some(event) = events.first()
            && event.transaction_hash.as_ref() != last_seen_tx.as_ref()
        {
            if let Some(tx) = &event.transaction_hash {
                *last_seen_tx = Some(tx.clone());
            }
            runtime.handle.emit(MonEvent::ActivityHit {
                leader: wallet.to_string(),
                asset: event.asset.clone(),
                condition: event.condition_id.clone().unwrap_or_default(),
                side: side_from_str(event.side.as_deref().unwrap_or("BUY")),
                usdc_size: (event.usdc_size * 1_000_000.0).round() as i64,
                leader_price_ppm: event
                    .price
                    .map(|value| (value * 1_000_000.0).round() as i32)
                    .unwrap_or(0),
                event_ts_ms: (event.timestamp as i64) * 1000,
                recv_ts_ms: now_ms_u64() as i64,
                tx_hash: event.transaction_hash.clone().unwrap_or_default(),
                slug: event.slug.clone(),
            });
        }
        if let Some(event) = events.first() {
            values.insert("latest_new_asset".to_string(), event.asset.clone());
        }
    }
    Ok(values)
}

fn build_watch_args(root: &Path, wallet: &str, proxy: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--root".to_string(),
        root.display().to_string(),
        "--user".to_string(),
        wallet.to_string(),
        "--poll-count".to_string(),
        "1".to_string(),
    ];
    if let Some(proxy) = proxy {
        args.push("--proxy".to_string());
        args.push(proxy.to_string());
    }
    args
}

fn run_position_cycle(
    runtime: &MonitorRuntime,
    position_bin: &Path,
    root: &Path,
    wallet: &str,
    focus_asset: Option<&str>,
) -> Result<(), String> {
    runtime.handle.emit(MonEvent::ReconcileStart {
        leader: wallet.to_string(),
    });
    let mut args = vec!["--root".to_string(), root.display().to_string()];
    if let Some(focus_asset) = focus_asset {
        args.push("--focus-asset".to_string());
        args.push(focus_asset.to_string());
    }
    let started = std::time::Instant::now();
    let output = match run_command(position_bin, &args, Some(Path::new("."))) {
        Ok(output) => output,
        Err(error)
            if focus_asset.is_some()
                && (error.contains("unknown argument: --focus-asset")
                    || error.contains("No such file or directory")) =>
        {
            run_position_via_cargo(&args)?
        }
        Err(error) => return Err(error),
    };
    let elapsed_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("position targeting stdout was not utf-8: {error}"))?;
    let values = parse_key_values(&stdout);
    let target_count = values
        .get("target_count")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let delta_count = values
        .get("delta_count")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let stale_asset_count = values
        .get("diagnostic_stale_asset_count")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let blocked_asset_count = values
        .get("diagnostic_blocked_asset_count")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let blocker_summary = values
        .get("diagnostic_blocker_summary")
        .cloned()
        .unwrap_or_else(|| "none".to_string());

    let positions_count = values
        .get("leader_position_count")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let value_usdc = values
        .get("leader_spot_value_usdc")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    runtime.handle.emit(MonEvent::ReconcileDone {
        leader: wallet.to_string(),
        ok: true,
        latency_ms: elapsed_ms,
        positions: positions_count,
        value_usdc,
        snapshot_age_ms: 0,
        provisional_drift_bps: 0,
    });

    runtime.handle.emit(MonEvent::PositionDiagnostics {
        target_count,
        delta_count,
        stale_asset_count,
        blocked_asset_count,
        blocker_summary: blocker_summary.clone(),
    });

    if let Some(asset) = values.get("focus.asset") {
        runtime.handle.emit(MonEvent::TrackedActivityProjection {
            asset: asset.clone(),
            current_position_value_usdc: values
                .get("focus.position_value_usdc")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
            current_position_size: values
                .get("focus.position_size")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
            current_avg_price_ppm: values
                .get("focus.avg_price_ppm")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0),
            algo_target_risk_usdc: values
                .get("focus.target_risk_usdc")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
            algo_delta_risk_usdc: values
                .get("focus.delta_risk_usdc")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
            algo_confidence_bps: values
                .get("focus.confidence_bps")
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(0),
            algo_tte_bucket: values
                .get("focus.tte_bucket")
                .cloned()
                .unwrap_or_else(|| "none".to_string()),
            algo_reason: values
                .get("focus.reason")
                .cloned()
                .unwrap_or_else(|| "none".to_string()),
        });
    } else if let Some(focus_asset) = focus_asset {
        runtime.handle.emit(MonEvent::TrackedActivityProjection {
            asset: focus_asset.to_string(),
            current_position_value_usdc: 0,
            current_position_size: 0,
            current_avg_price_ppm: 0,
            algo_target_risk_usdc: 0,
            algo_delta_risk_usdc: 0,
            algo_confidence_bps: 0,
            algo_tte_bucket: "none".to_string(),
            algo_reason: "asset_missing_in_positions_snapshot".to_string(),
        });
    }

    if let Some(asset) = values
        .get("target[0].slug")
        .or_else(|| values.get("target[0].asset"))
    {
        let total_target = values
            .get("diagnostic_total_target_risk_usdc")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        if delta_count == 0 && blocker_summary != "none" && blocker_summary != "none_detected" {
            runtime.handle.emit(MonEvent::SignalSkipped {
                asset: asset.clone(),
                reason: parse_skip_reason(&blocker_summary),
                fresh_ms: 0,
            });
        } else {
            runtime.handle.emit(MonEvent::SignalPlanned {
                asset: asset.clone(),
                leaders: 1,
                fresh_ms: 0,
                agree_bps: 10_000,
                raw_target_usdc: total_target,
                final_target_usdc: total_target,
            });
        }
    }

    let discovery_dir = root.join(".omx/discovery");
    let positions_path =
        discovery_dir.join(format!("positions-{}.json", sanitize_for_filename(wallet)));
    if positions_path.exists() {
        let body = fs::read_to_string(&positions_path)
            .map_err(|error| format!("failed to read {}: {error}", positions_path.display()))?;
        let positions = parse_leader_positions_payload(&body);
        for position in positions.iter().take(12) {
            let bid = ((position.avg_price_ppm as f64) * 0.998).round() as i32;
            let ask = ((position.avg_price_ppm as f64) * 1.002).round() as i32;
            runtime.handle.emit(MonEvent::BookUpdate {
                asset: position.slug.clone(),
                best_bid_ppm: bid.max(1),
                best_ask_ppm: ask.max(1),
                age_ms: 0,
                levels_bid: 1,
                levels_ask: 1,
                crossed: false,
                hash_mismatch: false,
            });
        }
    }

    emit_risk_snapshot(runtime, root, wallet)?;
    Ok(())
}

fn emit_risk_snapshot(runtime: &MonitorRuntime, root: &Path, wallet: &str) -> Result<(), String> {
    let discovery_dir = root.join(".omx/discovery");
    let positions_path =
        discovery_dir.join(format!("positions-{}.json", sanitize_for_filename(wallet)));
    let value_path = discovery_dir.join(format!("value-{}.json", sanitize_for_filename(wallet)));
    let positions_body = fs::read_to_string(&positions_path)
        .map_err(|error| format!("failed to read {}: {error}", positions_path.display()))?;
    let positions = parse_position_records(&positions_body);
    let total_value = fs::read_to_string(&value_path)
        .ok()
        .and_then(|body| parse_total_value(&body))
        .unwrap_or_else(|| {
            positions
                .iter()
                .map(|p| p.current_value.max(0.0))
                .sum::<f64>()
        });
    let now_sec = now_ms_u64() / 1000;
    let mut gross = 0i64;
    let mut tail_24h = 0i64;
    let mut tail_72h = 0i64;
    let mut neg_risk = 0i64;
    for position in positions {
        let current_usdc = (position.current_value.max(0.0) * 1_000_000.0).round() as i64;
        gross = gross.saturating_add(current_usdc);
        if position.negative_risk {
            neg_risk = neg_risk.saturating_add(current_usdc);
        }
        if let Some(end_date) = position
            .end_date
            .as_deref()
            .and_then(parse_iso8601_timestamp)
        {
            let remaining = end_date.saturating_sub(now_sec);
            if remaining <= 24 * 60 * 60 {
                tail_24h = tail_24h.saturating_add(current_usdc);
            }
            if remaining <= 72 * 60 * 60 {
                tail_72h = tail_72h.saturating_add(current_usdc);
            }
        }
    }
    let equity_usdc = (total_value * 1_000_000.0).round() as i64;
    runtime.handle.emit(MonEvent::RiskSnapshot {
        equity_usdc,
        cash_usdc: equity_usdc.saturating_sub(gross).max(0),
        deployed_usdc: gross,
        gross_usdc: gross,
        net_usdc: gross,
        tail_24h_usdc: tail_24h,
        tail_72h_usdc: tail_72h,
        neg_risk_usdc: neg_risk,
        tracking_err_bps: 0,
        hhi_bps: 0,
        follow_ratio_bps: 0,
    });
    Ok(())
}

fn emit_operator_exec_metrics(
    runtime: &MonitorRuntime,
    root: &Path,
    last_signature: &mut Option<String>,
) -> Result<(), String> {
    let latest = pick_operator_report(root);
    let Some(path) = latest else {
        return Ok(());
    };
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if Some(content.clone()) == *last_signature {
        return Ok(());
    }
    *last_signature = Some(content.clone());
    let values = parse_key_values(&content);
    let submit_ms = values
        .get("replay_submit_elapsed_ms")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let verify_ms = values
        .get("replay_verified_elapsed_ms")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let total_ms = values
        .get("last_total_elapsed_ms")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let status = values
        .get("last_submit_status")
        .map(String::as_str)
        .unwrap_or("none");
    runtime.handle.emit(MonEvent::OrderPosted {
        order_id: 1,
        latency_ms: submit_ms,
    });
    if matches!(status, "verified" | "processed") {
        runtime.handle.emit(MonEvent::OrderMatched {
            order_id: 1,
            matched_usdc: 1_000_000,
            matched_shares: 1_000_000,
            eff_px_ppm: 500_000,
            fee_usdc: 0,
            copy_gap_bps: 0,
            slip_bps: 0,
            latency_ms: verify_ms,
        });
        runtime.handle.emit(MonEvent::OrderConfirmed {
            order_id: 1,
            latency_ms: total_ms,
        });
    } else if status != "none" {
        runtime.handle.emit(MonEvent::OrderRejected {
            order_id: 1,
            reason: reject_reason_from_status(status),
        });
    }
    Ok(())
}

fn run_position_via_cargo(args: &[String]) -> Result<Output, String> {
    let mut cargo_args = vec![
        "run".to_string(),
        "--quiet".to_string(),
        "--bin".to_string(),
        "run_position_targeting_demo".to_string(),
        "--".to_string(),
    ];
    cargo_args.extend(args.iter().cloned());
    run_command(Path::new("cargo"), &cargo_args, Some(Path::new(".")))
}

fn parse_key_values(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn parse_skip_reason(summary: &str) -> SkipReason {
    let reason = summary.split(',').next().unwrap_or_default();
    let key = reason.split(':').next().unwrap_or_default();
    match key {
        "stale_target" => SkipReason::StaleSignal,
        "tail_lt24h" | "tail_lt72h" => SkipReason::TailWindow,
        "neg_risk" => SkipReason::NegRiskBlocked,
        "closed_or_book_off" => SkipReason::StaleBook,
        "low_copyable_liquidity" => SkipReason::NoLiquidity,
        "zero_target" | "projected_to_zero" => SkipReason::RiskCap,
        "below_min_effective_order" => SkipReason::CashCap,
        _ => SkipReason::NoLiquidity,
    }
}

fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn pick_operator_report(root: &Path) -> Option<PathBuf> {
    let latest = root.join(".omx/operator-demo/latest.txt");
    if latest.exists() {
        return Some(latest);
    }
    let dir = root.join(".omx/operator-demo");
    let mut entries = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    entries.last().map(|entry| entry.path())
}

fn resolve_bin_path(binary_name: &str, override_path: Option<&str>) -> io::Result<PathBuf> {
    if let Some(override_path) = override_path {
        return Ok(PathBuf::from(override_path));
    }
    let current = env::current_exe()?;
    let current_dir = current
        .parent()
        .ok_or_else(|| io::Error::other("current exe has no parent directory"))?;
    let direct = current_dir.join(binary_name);
    if direct.exists() {
        return Ok(direct);
    }
    if current_dir.ends_with("deps") {
        let sibling = current_dir
            .parent()
            .ok_or_else(|| io::Error::other("deps dir has no parent"))?
            .join(binary_name);
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    Ok(direct)
}

fn run_command(program: &Path, args: &[String], cwd: Option<&Path>) -> Result<Output, String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to execute {}: {error}", program.display()))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{} exited with {}: {} {}",
            program.display(),
            output.status.code().unwrap_or(1),
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, run_monitor};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-monitor-v1-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_monitor_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--summary".into(),
            "../.omx/discovery/wallet-filter-v1-summary.txt".into(),
            "--poll-interval-ms".into(),
            "2000".into(),
            "--reconcile-interval-ms".into(),
            "6000".into(),
            "--ui-refresh-ms".into(),
            "500".into(),
            "--iterations".into(),
            "2".into(),
            "--once".into(),
            "--no-http".into(),
        ])
        .expect("options parsed");

        assert_eq!(options.root, "..");
        assert_eq!(options.iterations, Some(1));
        assert!(options.once);
        assert!(!options.http_enabled);
    }

    #[test]
    fn run_monitor_builds_snapshot_from_mocked_watch_and_position_tools() {
        let root = unique_temp_dir("success");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("dir created");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xleader\nCOPYTRADER_SELECTED_SCORE=84\nCOPYTRADER_SELECTED_CATEGORY=TECH\nCOPYTRADER_SELECTED_REVIEW_STATUS=stable\n",
        )
        .expect("env written");
        fs::write(
            root.join(".omx/operator-demo/latest.txt"),
            "last_submit_status=verified\nreplay_submit_elapsed_ms=10\nreplay_verified_elapsed_ms=20\nlast_total_elapsed_ms=30\n",
        )
        .expect("operator latest written");
        fs::write(
            root.join(".omx/discovery/positions-0xleader.json"),
            r#"[{"proxyWallet":"0xleader","asset":"asset-1","conditionId":"cond-1","size":10,"avgPrice":0.42,"initialValue":4.2,"currentValue":5.5,"slug":"market-a","eventId":"event-1","outcome":"Yes","endDate":"2099-01-01","negativeRisk":false}]"#,
        )
        .expect("positions written");
        fs::write(
            root.join(".omx/discovery/value-0xleader.json"),
            r#"[{"value":5.5}]"#,
        )
        .expect("value written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/0xleader'\ncat > '{root}/.omx/live-activity/0xleader/latest-activity.json' <<'JSON'\n[{{\"proxyWallet\":\"0xleader\",\"timestamp\":1776500000,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"usdcSize\":12.5,\"transactionHash\":\"0xtx\",\"price\":0.44,\"side\":\"BUY\",\"conditionId\":\"cond-1\",\"slug\":\"market-a\"}}]\nJSON\nprintf 'watch_latest_path={root}/.omx/live-activity/0xleader/latest-activity.json\nlatest_new_slug=market-a\n'\n",
                root = root.display()
            ),
        );

        let position = root.join("run_position_targeting_demo");
        write_executable(
            &position,
            "#!/bin/sh\nprintf 'leader_position_count=1\nleader_spot_value_usdc=5500000\ntarget_count=1\ndelta_count=0\ndiagnostic_total_target_risk_usdc=2000000\ndiagnostic_stale_asset_count=1\ndiagnostic_blocked_asset_count=1\ndiagnostic_blocker_summary=zero_target:1,tail_lt24h:1\ntarget[0].asset=asset-1\ntarget[0].slug=market-a\nfocus.asset=asset-1\nfocus.slug=market-a\nfocus.position_value_usdc=5500000\nfocus.position_size=10000000\nfocus.avg_price_ppm=440000\nfocus.target_risk_usdc=0\nfocus.delta_risk_usdc=0\nfocus.confidence_bps=10000\nfocus.tte_bucket=none\nfocus.reason=risk_cap\n'\n",
        );

        let options = super::Options {
            root: root.display().to_string(),
            summary_path: None,
            selected_leader_env: Some(
                root.join(".omx/discovery/selected-leader.env")
                    .display()
                    .to_string(),
            ),
            proxy: None,
            poll_interval_ms: 100,
            reconcile_interval_ms: 100,
            ui_refresh_ms: 100,
            iterations: Some(1),
            once: true,
            auto_select: false,
            http_enabled: false,
            http_bind: "127.0.0.1:0".to_string(),
            select_bin: None,
            watch_bin: Some(watch.display().to_string()),
            position_targeting_bin: Some(position.display().to_string()),
        };

        let frame = run_monitor(&options).expect("monitor should succeed");
        assert!(frame.contains("HEALTH="));
        assert!(frame.contains("0xleader"));
        assert!(frame.contains("market-a"));
        assert!(frame.contains("selected="));
        assert!(frame.contains("category=TECH"));
        assert!(frame.contains("TRADE TAPE"));
        assert!(frame.contains("asset=asset-1"));
        assert!(frame.contains("time="));
        assert!(frame.contains("price=0.4400") || frame.contains("px=0.4400"));
        assert!(frame.contains("leader_pos=5.50"));
        assert!(frame.contains("algo_target=0.00"));
        assert!(frame.contains("reason=risk_cap"));
        assert!(frame.contains("market-a usdc=12.50"));
        assert!(frame.contains("SKIP risk_cap"));
        assert!(frame.contains("SIGNALS"));
        assert!(frame.contains("blocker_summary="));
        assert!(frame.contains("target_count=1 delta_count=0"));
        assert!(frame.contains("RISK"));
        assert!(frame.contains("TRACKING"));
        assert!(frame.contains("SKIP"));
        assert!(root.join(".omx/monitor/latest.txt").exists());
        assert!(root.join(".omx/monitor/metrics.txt").exists());
        assert!(root.join(".omx/monitor/health.json").exists());
    }
}
