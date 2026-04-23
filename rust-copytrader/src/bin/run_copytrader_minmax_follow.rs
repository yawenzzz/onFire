use rust_copytrader::config::is_valid_evm_wallet;
use rust_copytrader::wallet_filter::{
    ActivityRecord, parse_activity_records, select_activity_record_json,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
struct Options {
    root: String,
    user: Option<String>,
    selected_leader_env: Option<String>,
    proxy: Option<String>,
    watch_limit: usize,
    loop_count: usize,
    forever: bool,
    loop_interval_ms: u64,
    min_open_usdc: f64,
    max_open_usdc: f64,
    flat_score: u8,
    max_total_exposure_usdc: Option<String>,
    max_order_usdc: Option<String>,
    account_snapshot: Option<String>,
    account_snapshot_max_age_secs: u64,
    activity_max_age_secs: u64,
    allow_live_submit: bool,
    force_live_submit: bool,
    ignore_seen_tx: bool,
    activity_source_verified: bool,
    activity_under_budget: bool,
    activity_capability_detected: bool,
    positions_under_budget: bool,
    watch_bin: Option<String>,
    live_submit_bin: Option<String>,
    account_monitor_bin: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            user: None,
            selected_leader_env: None,
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            watch_limit: 50,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 500,
            min_open_usdc: 0.1,
            max_open_usdc: 10.0,
            flat_score: 50,
            max_total_exposure_usdc: None,
            max_order_usdc: None,
            account_snapshot: None,
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: env::var("COPYTRADER_ACTIVITY_MAX_AGE_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(60),
            allow_live_submit: false,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: None,
            live_submit_bin: None,
            account_monitor_bin: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SizedActivity {
    tx: String,
    timestamp: u64,
    side: String,
    asset: String,
    condition_id: Option<String>,
    outcome: Option<String>,
    slug: Option<String>,
    size: f64,
    usdc_size: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct ConditionSizingDecision {
    condition_key: String,
    outcome_label: String,
    same_outcome_count: usize,
    same_outcome_sum_usdc: f64,
    opposite_outcome_count: usize,
    opposite_outcome_sum_usdc: f64,
    total_condition_sum_usdc: f64,
    decision_tag: &'static str,
    reason: &'static str,
    should_follow: bool,
    recommended_open_usdc: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct SellInventoryDecision {
    inventory_net_size: Option<f64>,
    sellable_usdc: Option<f64>,
    decision_tag: &'static str,
    reason: &'static str,
    should_follow: bool,
    adjusted_open_usdc: f64,
}

const MIN_NEW_CONDITION_ENTRY_USDC: f64 = 5.0;
const OPPOSITE_OUTCOME_HEDGE_RATIO: f64 = 0.35;
const FOLLOW_LATEST_TRADE_RATIO: f64 = 0.10;
const FOLLOW_SAME_OUTCOME_SUM_RATIO: f64 = 0.02;

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

    match run_minmax_follow(&options) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
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
        "usage: run_copytrader_minmax_follow [--root <path>] [--user <wallet>] [--selected-leader-env <path>] [--proxy <url>] [--watch-limit <n>] [--loop-count <n>] [--forever] [--loop-interval-ms <n>] [--min-open-usdc <decimal>] [--max-open-usdc <decimal>] [--flat-score <1..100>] [--max-total-exposure-usdc <decimal>] [--max-order-usdc <decimal>] [--account-snapshot <path>] [--account-snapshot-max-age-secs <n>] [--activity-max-age-secs <n>] [--allow-live-submit] [--force-live-submit] [--ignore-seen-tx] [--activity-source-verified] [--activity-under-budget] [--activity-capability-detected] [--positions-under-budget] [--watch-bin <path>] [--live-submit-bin <path>] [--account-monitor-bin <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--user" => options.user = Some(next_value(&mut iter, arg)?),
            "--selected-leader-env" => {
                options.selected_leader_env = Some(next_value(&mut iter, arg)?)
            }
            "--proxy" => options.proxy = Some(next_value(&mut iter, arg)?),
            "--watch-limit" => {
                options.watch_limit = parse_usize(&next_value(&mut iter, arg)?, "watch-limit")?
            }
            "--loop-count" => {
                options.loop_count = parse_usize(&next_value(&mut iter, arg)?, "loop-count")?
            }
            "--forever" => options.forever = true,
            "--loop-interval-ms" => {
                options.loop_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "loop-interval-ms")?
            }
            "--min-open-usdc" => {
                options.min_open_usdc = parse_f64(&next_value(&mut iter, arg)?, "min-open-usdc")?
            }
            "--max-open-usdc" => {
                options.max_open_usdc = parse_f64(&next_value(&mut iter, arg)?, "max-open-usdc")?
            }
            "--flat-score" => {
                options.flat_score = parse_u8(&next_value(&mut iter, arg)?, "flat-score")?
            }
            "--max-total-exposure-usdc" => {
                let value = next_value(&mut iter, arg)?;
                parse_f64(&value, "max-total-exposure-usdc")?;
                options.max_total_exposure_usdc = Some(value);
            }
            "--max-order-usdc" => {
                let value = next_value(&mut iter, arg)?;
                parse_f64(&value, "max-order-usdc")?;
                options.max_order_usdc = Some(value);
            }
            "--account-snapshot" => options.account_snapshot = Some(next_value(&mut iter, arg)?),
            "--account-snapshot-max-age-secs" => {
                options.account_snapshot_max_age_secs = parse_u64(
                    &next_value(&mut iter, arg)?,
                    "account-snapshot-max-age-secs",
                )?
            }
            "--activity-max-age-secs" => {
                options.activity_max_age_secs =
                    parse_u64(&next_value(&mut iter, arg)?, "activity-max-age-secs")?
            }
            "--allow-live-submit" => options.allow_live_submit = true,
            "--force-live-submit" => {
                options.force_live_submit = true;
                options.allow_live_submit = true;
            }
            "--ignore-seen-tx" => options.ignore_seen_tx = true,
            "--activity-source-verified" => options.activity_source_verified = true,
            "--activity-under-budget" => options.activity_under_budget = true,
            "--activity-capability-detected" => options.activity_capability_detected = true,
            "--positions-under-budget" => options.positions_under_budget = true,
            "--watch-bin" => options.watch_bin = Some(next_value(&mut iter, arg)?),
            "--live-submit-bin" => options.live_submit_bin = Some(next_value(&mut iter, arg)?),
            "--account-monitor-bin" => {
                options.account_monitor_bin = Some(next_value(&mut iter, arg)?)
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if options.flat_score == 0 || options.flat_score > 100 {
        return Err("flat-score must be between 1 and 100".to_string());
    }
    if options.min_open_usdc <= 0.0 || options.max_open_usdc < options.min_open_usdc {
        return Err("require 0 < min-open-usdc <= max-open-usdc".to_string());
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

fn parse_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_u8(value: &str, field: &str) -> Result<u8, String> {
    value
        .parse::<u8>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_f64(value: &str, field: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("invalid decimal for {field}: {value}"))
}

fn run_minmax_follow(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let watch_bin = resolve_bin_path("watch_copy_leader_activity", options.watch_bin.as_deref())
        .map_err(|error| format!("failed to resolve watch_copy_leader_activity: {error}"))?;
    let live_submit_bin = resolve_bin_path(
        "run_copytrader_live_submit_gate",
        options.live_submit_bin.as_deref(),
    )
    .map_err(|error| format!("failed to resolve run_copytrader_live_submit_gate: {error}"))?;
    let selected_env = options
        .selected_leader_env
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery/selected-leader.env"));
    let wallet = options
        .user
        .clone()
        .or_else(|| read_selected_leader_wallet(&selected_env).ok())
        .ok_or_else(|| "missing --user and no selected leader env found".to_string())?;
    if !is_valid_evm_wallet(&wallet) {
        return Err(format!("invalid live-follow wallet: {wallet}"));
    }

    let state_root = root
        .join(".omx")
        .join("minmax-follow")
        .join(sanitize_for_filename(&wallet));
    fs::create_dir_all(&state_root)
        .map_err(|error| format!("failed to create {}: {error}", state_root.display()))?;

    let strategy_env = state_root.join("selected-leader.env");
    if options.user.is_some() || !selected_env.exists() {
        write_strategy_env(&strategy_env, &wallet)?;
    }
    let submit_selected_env = if selected_env.exists() && options.user.is_none() {
        selected_env.clone()
    } else {
        strategy_env.clone()
    };

    let latest_activity_path = root
        .join(".omx/live-activity")
        .join(sanitize_for_filename(&wallet))
        .join("latest-activity.json");
    let seen_path = state_root.join("submitted-tx.txt");
    let mut seen = read_seen_txs(&seen_path)
        .map_err(|error| format!("failed to read {}: {error}", seen_path.display()))?;

    let loop_total = options.loop_count.max(1);
    let mut index = 0usize;
    let last_lines = loop {
        let watch_started_at_unix_ms = current_unix_ms()?;
        let watch_lines = run_watch_once(&watch_bin, options, &wallet)?;
        let watch_finished_at_unix_ms = current_unix_ms()?;
        let activities = read_recent_trade_activity(&latest_activity_path)?;
        let latest_new_tx = extract_metric_string(&watch_lines, "latest_new_tx");
        let latest = latest_trade_for_watch(&activities, latest_new_tx.as_deref())?;
        let score =
            compute_normalized_score(&activities, latest.usdc_size.abs(), options.flat_score);
        let normalized_open_usdc =
            map_score_to_open_usdc(score, options.min_open_usdc, options.max_open_usdc);
        let condition_decision =
            decide_condition_sizing(&activities, latest, normalized_open_usdc, options);
        let history = history_min_max(&activities);
        let run_id = now_nanos()?;
        let report_path = state_root.join(format!("run-{}-{run_id}.txt", index));
        let selected_latest_activity_path =
            state_root.join(format!("run-{}-{run_id}.selected-latest-activity.json", index));
        write_selected_activity_file(&latest_activity_path, &selected_latest_activity_path, &latest.tx)?;

        let mut lines = vec![
            "strategy=minmax_activity_v1".to_string(),
            format!("leader_wallet={wallet}"),
            format!("auto_submit_enabled={}", options.allow_live_submit),
            format!("force_live_submit={}", options.force_live_submit),
            format!("ignore_seen_tx={}", options.ignore_seen_tx),
            format!(
                "submit_mode={}",
                if options.allow_live_submit {
                    "live"
                } else {
                    "preview"
                }
            ),
            format!("latest_activity_path={}", latest_activity_path.display()),
            format!(
                "selected_latest_activity_path={}",
                selected_latest_activity_path.display()
            ),
            format!("selected_leader_env_path={}", submit_selected_env.display()),
            format!("latest_tx={}", latest.tx),
            format!("latest_timestamp={}", latest.timestamp),
            format!("latest_side={}", latest.side),
            format!(
                "latest_slug={}",
                latest.slug.as_deref().unwrap_or("unknown")
            ),
            format!("watch_started_at_unix_ms={watch_started_at_unix_ms}"),
            format!("watch_finished_at_unix_ms={watch_finished_at_unix_ms}"),
            format!(
                "watch_elapsed_ms={}",
                watch_finished_at_unix_ms.saturating_sub(watch_started_at_unix_ms)
            ),
            format!(
                "leader_to_watch_finished_ms={}",
                watch_finished_at_unix_ms.saturating_sub(latest.timestamp.saturating_mul(1000))
            ),
            format!("latest_asset={}", latest.asset),
            format!("latest_activity_usdc={:.6}", latest.usdc_size),
            format!("recent_trade_count={}", activities.len()),
            format!("recent_usdc_min={:.6}", history.0),
            format!("recent_usdc_max={:.6}", history.1),
            format!("normalized_score={score}"),
            format!("condition_key={}", condition_decision.condition_key),
            format!("condition_outcome={}", condition_decision.outcome_label),
            format!(
                "condition_same_outcome_count={}",
                condition_decision.same_outcome_count
            ),
            format!(
                "condition_same_outcome_sum_usdc={:.6}",
                condition_decision.same_outcome_sum_usdc
            ),
            format!(
                "condition_opposite_outcome_count={}",
                condition_decision.opposite_outcome_count
            ),
            format!(
                "condition_opposite_outcome_sum_usdc={:.6}",
                condition_decision.opposite_outcome_sum_usdc
            ),
            format!(
                "condition_total_sum_usdc={:.6}",
                condition_decision.total_condition_sum_usdc
            ),
            format!("condition_decision={}", condition_decision.decision_tag),
            format!("condition_decision_reason={}", condition_decision.reason),
            format!(
                "condition_should_follow={}",
                condition_decision.should_follow
            ),
            format!("normalized_open_usdc={normalized_open_usdc:.6}"),
        ];
        lines.extend(watch_lines.into_iter().filter(|line| {
            line.starts_with("watch_")
                || line.starts_with("poll_")
                || line.starts_with("latest_new_")
        }));
        let poll_new_events = extract_metric_usize(&lines, "poll_new_events");
        let latest_new_timestamp = extract_metric_u64(&lines, "latest_new_timestamp");
        let latest_new_age_secs = latest_new_timestamp
            .map(|timestamp| watch_finished_at_unix_ms.saturating_sub(timestamp.saturating_mul(1000)) / 1000);
        lines.push(format!(
            "watch_has_new_activity={}",
            poll_new_events.is_some_and(|count| count > 0)
        ));
        lines.push(format!(
            "watch_latest_new_activity_age_secs={}",
            latest_new_age_secs
                .map(|value| value.to_string())
                .unwrap_or_default()
        ));
        lines.push(format!(
            "watch_latest_new_activity_under_budget={}",
            latest_new_age_secs
                .map(|value| value <= options.activity_max_age_secs)
                .unwrap_or(false)
        ));
        let refresh_lines = run_account_snapshot_refresh(&root, options);
        let refresh_ok = refresh_lines
            .iter()
            .any(|line| line == "account_snapshot_refresh_status=ok");
        lines.extend(refresh_lines);
        let sell_inventory_decision = evaluate_sell_inventory(
            &root,
            options,
            latest,
            refresh_ok,
            condition_decision.recommended_open_usdc,
        )?;
        let open_usdc = sell_inventory_decision.adjusted_open_usdc;
        lines.push(format!(
            "sell_inventory_net_size={}",
            sell_inventory_decision
                .inventory_net_size
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default()
        ));
        lines.push(format!(
            "sell_inventory_sellable_usdc={}",
            sell_inventory_decision
                .sellable_usdc
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default()
        ));
        lines.push(format!(
            "sell_inventory_decision={}",
            sell_inventory_decision.decision_tag
        ));
        lines.push(format!(
            "sell_inventory_reason={}",
            sell_inventory_decision.reason
        ));
        lines.push(format!(
            "sell_inventory_should_follow={}",
            sell_inventory_decision.should_follow
        ));
        lines.push(format!("planned_open_usdc={open_usdc:.6}"));

        if !options.force_live_submit && poll_new_events == Some(0) {
            lines.push("submit_status=skipped_no_new_activity".to_string());
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else if !options.force_live_submit
            && poll_new_events.is_some_and(|count| count > 0)
            && latest_new_age_secs.is_some_and(|age| age > options.activity_max_age_secs)
        {
            lines.push("submit_status=skipped_stale_new_activity".to_string());
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else if !condition_decision.should_follow {
            lines.push(format!(
                "submit_status=skipped_{}",
                condition_decision.decision_tag
            ));
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else if !sell_inventory_decision.should_follow {
            lines.push(format!(
                "submit_status=skipped_{}",
                sell_inventory_decision.decision_tag
            ));
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else if options.allow_live_submit && !refresh_ok {
            lines.push("submit_status=skipped_account_snapshot_refresh_failed".to_string());
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else if seen.contains(&latest.tx) && !options.ignore_seen_tx {
            lines.push("submit_status=skipped_duplicate_tx".to_string());
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        } else {
            let submit_output = run_live_submit(
                &live_submit_bin,
                &root,
                &selected_latest_activity_path,
                &submit_selected_env,
                open_usdc,
                options,
            )?;
            let submitted = submit_output_contains_status(&submit_output, "submitted");
            if submit_output_marks_seen(&submit_output) {
                seen.insert(latest.tx.clone());
                write_seen_txs(&seen_path, &seen)
                    .map_err(|error| format!("failed to write {}: {error}", seen_path.display()))?;
            }
            lines.extend(
                submit_output
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToString::to_string),
            );
            if submit_output_contains_prefix(&submit_output, "live_gate_status=blocked:") {
                lines.push("submit_status=skipped_live_gate_blocked".to_string());
            }
            if submitted {
                lines.extend(run_post_submit_account_snapshot_refresh(&root, options));
                lines.push("post_submit_user_probe_status=disabled".to_string());
            }
            lines.push(format!("report_path={}", report_path.display()));
            persist_iteration_report(&state_root, &report_path, &lines)?;
            if !options.forever && index + 1 >= loop_total {
                break lines;
            }
        }

        index = index.saturating_add(1);
        if options.loop_interval_ms > 0 {
            thread::sleep(Duration::from_millis(options.loop_interval_ms));
        }
    };
    Ok(last_lines)
}

fn run_watch_once(
    watch_bin: &Path,
    options: &Options,
    wallet: &str,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "--root".to_string(),
        options.root.clone(),
        "--user".to_string(),
        wallet.to_string(),
        "--limit".to_string(),
        options.watch_limit.to_string(),
        "--poll-count".to_string(),
        "1".to_string(),
    ];
    if let Some(proxy) = &options.proxy {
        args.push("--proxy".to_string());
        args.push(proxy.clone());
    }
    let output = run_command(watch_bin, &args, Some(Path::new(".")))?;
    String::from_utf8(output.stdout)
        .map_err(|error| format!("watch_copy_leader_activity stdout was not utf-8: {error}"))
        .map(|stdout| {
            stdout
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect()
        })
}

fn run_live_submit(
    live_submit_bin: &Path,
    root: &Path,
    latest_activity_path: &Path,
    selected_env: &Path,
    open_usdc: f64,
    options: &Options,
) -> Result<String, String> {
    let mut args = vec![
        "--root".to_string(),
        root.display().to_string(),
        "--latest-activity".to_string(),
        latest_activity_path.display().to_string(),
        "--selected-leader-env".to_string(),
        selected_env.display().to_string(),
        "--override-usdc-size".to_string(),
        format!("{open_usdc:.6}"),
    ];
    if options.allow_live_submit {
        args.push("--allow-live-submit".to_string());
    }
    if options.force_live_submit {
        args.push("--force-live-submit".to_string());
    }
    if let Some(value) = &options.max_total_exposure_usdc {
        args.push("--max-total-exposure-usdc".to_string());
        args.push(value.clone());
    }
    if let Some(value) = &options.max_order_usdc {
        args.push("--max-order-usdc".to_string());
        args.push(value.clone());
    }
    if let Some(path) = &options.account_snapshot {
        args.push("--account-snapshot".to_string());
        args.push(path.clone());
    }
    if options.account_snapshot_max_age_secs > 0 {
        args.push("--account-snapshot-max-age-secs".to_string());
        args.push(options.account_snapshot_max_age_secs.to_string());
    }
    if options.activity_max_age_secs > 0 {
        args.push("--activity-max-age-secs".to_string());
        args.push(options.activity_max_age_secs.to_string());
    }
    let output = run_command(live_submit_bin, &args, Some(Path::new(".")))?;
    String::from_utf8(output.stdout)
        .map_err(|error| format!("run_copytrader_live_submit_gate stdout was not utf-8: {error}"))
}

fn write_selected_activity_file(
    latest_activity_path: &Path,
    selected_activity_path: &Path,
    tx: &str,
) -> Result<(), String> {
    let body = fs::read_to_string(latest_activity_path)
        .map_err(|error| format!("failed to read {}: {error}", latest_activity_path.display()))?;
    let selected = select_activity_record_json(&body, tx).ok_or_else(|| {
        format!(
            "failed to select tx {tx} from {}",
            latest_activity_path.display()
        )
    })?;
    if let Some(parent) = selected_activity_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(selected_activity_path, format!("{selected}\n"))
        .map_err(|error| format!("failed to write {}: {error}", selected_activity_path.display()))
}

fn run_account_snapshot_refresh(root: &Path, options: &Options) -> Vec<String> {
    run_snapshot_refresh_with_prefix(root, options, "account_snapshot_refresh")
}

fn run_post_submit_account_snapshot_refresh(root: &Path, options: &Options) -> Vec<String> {
    run_snapshot_refresh_with_prefix(root, options, "post_submit_account_snapshot_refresh")
}

fn run_snapshot_refresh_with_prefix(root: &Path, options: &Options, prefix: &str) -> Vec<String> {
    let Some(snapshot_path) = options.account_snapshot.as_deref().filter(|value| !value.is_empty()) else {
        return vec![format!("{prefix}_status=disabled:no_snapshot_path")];
    };
    let Some(account_monitor_bin) = options
        .account_monitor_bin
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return vec![format!("{prefix}_status=disabled:no_account_monitor_bin")];
    };

    let args = vec![
        account_monitor_bin.to_string(),
        "--output".to_string(),
        snapshot_path.to_string(),
    ];
    match run_command(Path::new("bash"), &args, Some(root)) {
        Ok(output) if output.status.success() => vec![
            format!("{prefix}_status=ok"),
            format!("{prefix}_output={snapshot_path}"),
        ],
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            vec![
                format!("{prefix}_status=failed"),
                format!("{prefix}_error={}", sanitize_report_text(&detail)),
            ]
        }
        Err(error) => vec![
            format!("{prefix}_status=failed"),
            format!("{prefix}_error={}", sanitize_report_text(&error)),
        ],
    }
}

fn evaluate_sell_inventory(
    root: &Path,
    options: &Options,
    latest: &SizedActivity,
    refresh_ok: bool,
    planned_open_usdc: f64,
) -> Result<SellInventoryDecision, String> {
    if !latest.side.eq_ignore_ascii_case("SELL") {
        return Ok(SellInventoryDecision {
            inventory_net_size: None,
            sellable_usdc: None,
            decision_tag: "not_sell",
            reason: "latest_side_not_sell",
            should_follow: true,
            adjusted_open_usdc: planned_open_usdc,
        });
    }

    if !refresh_ok {
        return Ok(SellInventoryDecision {
            inventory_net_size: None,
            sellable_usdc: None,
            decision_tag: "sell_inventory_unknown",
            reason: "account_snapshot_refresh_not_ok",
            should_follow: false,
            adjusted_open_usdc: planned_open_usdc,
        });
    }

    let Some(snapshot_path) = options.account_snapshot.as_deref().filter(|value| !value.is_empty()) else {
        return Ok(SellInventoryDecision {
            inventory_net_size: None,
            sellable_usdc: None,
            decision_tag: "sell_inventory_unknown",
            reason: "missing_account_snapshot_path",
            should_follow: false,
            adjusted_open_usdc: planned_open_usdc,
        });
    };

    let snapshot_path = root.join(snapshot_path);
    let inventory_net_size = read_asset_inventory_net_size(&snapshot_path, &latest.asset)?;
    let Some(inventory_net_size) = inventory_net_size else {
        return Ok(SellInventoryDecision {
            inventory_net_size: None,
            sellable_usdc: Some(0.0),
            decision_tag: "sell_without_inventory",
            reason: "asset_not_present_in_snapshot",
            should_follow: false,
            adjusted_open_usdc: 0.0,
        });
    };

    if inventory_net_size <= 0.0 {
        return Ok(SellInventoryDecision {
            inventory_net_size: Some(inventory_net_size),
            sellable_usdc: Some(0.0),
            decision_tag: "sell_without_inventory",
            reason: "inventory_net_size_non_positive",
            should_follow: false,
            adjusted_open_usdc: 0.0,
        });
    }

    let sellable_usdc = if latest.size > 0.0 && latest.usdc_size > 0.0 {
        latest.usdc_size * (inventory_net_size / latest.size)
    } else {
        0.0
    };

    if sellable_usdc <= 0.0 {
        return Ok(SellInventoryDecision {
            inventory_net_size: Some(inventory_net_size),
            sellable_usdc: Some(0.0),
            decision_tag: "sell_without_inventory",
            reason: "sellable_usdc_non_positive",
            should_follow: false,
            adjusted_open_usdc: 0.0,
        });
    }

    if sellable_usdc < planned_open_usdc {
        return Ok(SellInventoryDecision {
            inventory_net_size: Some(inventory_net_size),
            sellable_usdc: Some(sellable_usdc),
            decision_tag: "sell_inventory_capped",
            reason: "planned_sell_exceeds_inventory",
            should_follow: true,
            adjusted_open_usdc: sellable_usdc.min(planned_open_usdc),
        });
    }

    Ok(SellInventoryDecision {
        inventory_net_size: Some(inventory_net_size),
        sellable_usdc: Some(sellable_usdc),
        decision_tag: "sell_inventory_ok",
        reason: "inventory_covers_planned_sell",
        should_follow: true,
        adjusted_open_usdc: planned_open_usdc,
    })
}

fn read_asset_inventory_net_size(snapshot_path: &Path, asset_id: &str) -> Result<Option<f64>, String> {
    let body = fs::read_to_string(snapshot_path)
        .map_err(|error| format!("failed to read {}: {error}", snapshot_path.display()))?;
    let snapshot: Value = serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse {}: {error}", snapshot_path.display()))?;
    let positions = snapshot
        .get("account_snapshot")
        .and_then(|value| value.get("positions"))
        .or_else(|| snapshot.get("positions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for position in positions {
        let candidate_asset = position
            .get("asset_id")
            .or_else(|| position.get("asset"))
            .and_then(Value::as_str);
        if candidate_asset != Some(asset_id) {
            continue;
        }
        let net_size = position
            .get("net_size")
            .or_else(|| position.get("size"))
            .map(|value| {
                value
                    .as_str()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .ok_or_else(|| format!("missing net_size for asset {asset_id} in {}", snapshot_path.display()))?
            .parse::<f64>()
            .map_err(|error| format!("invalid net_size for asset {asset_id}: {error}"))?;
        return Ok(Some(net_size));
    }

    Ok(None)
}

fn submit_output_marks_seen(output: &str) -> bool {
    let submitted = output
        .lines()
        .any(|line| matches!(line.trim(), "live_submit_status=submitted"));
    if !submitted {
        return false;
    }
    !output
        .lines()
        .any(|line| matches!(line.trim(), "submit_success=false"))
}

fn sanitize_report_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn extract_metric_usize(lines: &[String], key: &str) -> Option<usize> {
    let prefix = format!("{key}=");
    lines.iter().find_map(|line| {
        line.strip_prefix(&prefix)
            .and_then(|value| value.parse::<usize>().ok())
    })
}

fn extract_metric_u64(lines: &[String], key: &str) -> Option<u64> {
    let prefix = format!("{key}=");
    lines.iter().find_map(|line| {
        line.strip_prefix(&prefix)
            .and_then(|value| value.parse::<u64>().ok())
    })
}

fn submit_output_contains_status(output: &str, status: &str) -> bool {
    let needle = format!("live_submit_status={status}");
    output.lines().any(|line| line.trim() == needle)
}

fn submit_output_contains_prefix(output: &str, prefix: &str) -> bool {
    output.lines().any(|line| line.trim().starts_with(prefix))
}

fn extract_metric_string(lines: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    lines.iter()
        .find_map(|line| line.strip_prefix(&prefix).map(ToString::to_string))
}

fn read_recent_trade_activity(path: &Path) -> Result<Vec<SizedActivity>, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let activities = parse_activity_records(&body)
        .into_iter()
        .filter_map(activity_to_sized)
        .collect::<Vec<_>>();
    if activities.is_empty() {
        return Err(format!("no trade activity found in {}", path.display()));
    }
    Ok(activities)
}

fn latest_trade_for_watch<'a>(
    activities: &'a [SizedActivity],
    latest_new_tx: Option<&str>,
) -> Result<&'a SizedActivity, String> {
    if let Some(latest_new_tx) = latest_new_tx
        && let Some(activity) = activities.iter().find(|activity| activity.tx == latest_new_tx)
    {
        return Ok(activity);
    }
    latest_trade(activities)
}

fn activity_to_sized(record: ActivityRecord) -> Option<SizedActivity> {
    if record.event_type != "TRADE" {
        return None;
    }
    let tx = record.transaction_hash?;
    let side = record.side?;
    Some(SizedActivity {
        tx,
        timestamp: record.timestamp,
        side,
        asset: record.asset,
        condition_id: record.condition_id,
        outcome: record.outcome,
        slug: record.slug,
        size: record.size.abs(),
        usdc_size: record.usdc_size.abs(),
    })
}

fn latest_trade(activities: &[SizedActivity]) -> Result<&SizedActivity, String> {
    activities
        .iter()
        .max_by_key(|activity| activity.timestamp)
        .ok_or_else(|| "missing latest trade".to_string())
}

fn history_min_max(activities: &[SizedActivity]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = 0.0_f64;
    for activity in activities {
        min = min.min(activity.usdc_size);
        max = max.max(activity.usdc_size);
    }
    if !min.is_finite() {
        (0.0, 0.0)
    } else {
        (min, max)
    }
}

fn compute_normalized_score(activities: &[SizedActivity], current_usdc: f64, flat_score: u8) -> u8 {
    let (min, max) = history_min_max(activities);
    if max <= min + f64::EPSILON {
        return flat_score;
    }
    let ratio = ((current_usdc - min) / (max - min)).clamp(0.0, 1.0);
    (1.0 + (ratio * 99.0).round()) as u8
}

fn map_score_to_open_usdc(score: u8, min_open_usdc: f64, max_open_usdc: f64) -> f64 {
    if (max_open_usdc - min_open_usdc).abs() <= f64::EPSILON {
        return min_open_usdc;
    }
    let ratio = (f64::from(score).saturating_sub(1.0)) / 99.0;
    min_open_usdc + ((max_open_usdc - min_open_usdc) * ratio)
}

fn decide_condition_sizing(
    activities: &[SizedActivity],
    latest: &SizedActivity,
    normalized_open_usdc: f64,
    options: &Options,
) -> ConditionSizingDecision {
    let condition_key = latest
        .condition_id
        .clone()
        .unwrap_or_else(|| latest.asset.clone());
    let outcome_label = latest
        .outcome
        .clone()
        .unwrap_or_else(|| latest.asset.clone());

    if latest.condition_id.is_none() && latest.outcome.is_none() {
        return ConditionSizingDecision {
            condition_key,
            outcome_label,
            same_outcome_count: 0,
            same_outcome_sum_usdc: 0.0,
            opposite_outcome_count: 0,
            opposite_outcome_sum_usdc: 0.0,
            total_condition_sum_usdc: 0.0,
            decision_tag: "normalized_fallback_follow",
            reason: "missing_condition_or_outcome",
            should_follow: true,
            recommended_open_usdc: normalized_open_usdc,
        };
    }

    let mut same_outcome_count = 0usize;
    let mut same_outcome_sum_usdc = 0.0_f64;
    let mut opposite_outcome_count = 0usize;
    let mut opposite_outcome_sum_usdc = 0.0_f64;

    for activity in activities {
        if activity_condition_key(activity) != condition_key {
            continue;
        }
        if same_condition_outcome(activity, latest) {
            same_outcome_count += 1;
            same_outcome_sum_usdc += activity.usdc_size;
        } else if opposite_condition_outcome(activity, latest) {
            opposite_outcome_count += 1;
            opposite_outcome_sum_usdc += activity.usdc_size;
        }
    }

    let total_condition_sum_usdc = same_outcome_sum_usdc + opposite_outcome_sum_usdc;
    let max_order_cap = options
        .max_order_usdc
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value.min(options.max_open_usdc))
        .unwrap_or(options.max_open_usdc);
    let recommended_open_usdc = max_order_cap
        .min(
            (latest.usdc_size * FOLLOW_LATEST_TRADE_RATIO)
                .max(same_outcome_sum_usdc * FOLLOW_SAME_OUTCOME_SUM_RATIO),
        )
        .max(options.min_open_usdc);

    let confirmed_same_outcome = latest.usdc_size >= MIN_NEW_CONDITION_ENTRY_USDC
        || same_outcome_sum_usdc >= MIN_NEW_CONDITION_ENTRY_USDC
        || (same_outcome_count >= 2 && same_outcome_sum_usdc >= options.min_open_usdc * 2.0);

    let (decision_tag, reason, should_follow) = if opposite_outcome_sum_usdc > 0.0
        && same_outcome_sum_usdc < opposite_outcome_sum_usdc * OPPOSITE_OUTCOME_HEDGE_RATIO
        && latest.usdc_size < MIN_NEW_CONDITION_ENTRY_USDC
    {
        (
            "condition_hedge_candidate",
            "opposite_outcome_dominates_recent_flow",
            false,
        )
    } else if !confirmed_same_outcome {
        (
            "condition_unconfirmed_small_entry",
            "insufficient_same_outcome_confirmation",
            false,
        )
    } else {
        ("condition_follow_confirmed", "same_outcome_confirmed", true)
    };

    ConditionSizingDecision {
        condition_key,
        outcome_label,
        same_outcome_count,
        same_outcome_sum_usdc,
        opposite_outcome_count,
        opposite_outcome_sum_usdc,
        total_condition_sum_usdc,
        decision_tag,
        reason,
        should_follow,
        recommended_open_usdc,
    }
}

fn activity_condition_key(activity: &SizedActivity) -> String {
    activity
        .condition_id
        .clone()
        .unwrap_or_else(|| activity.asset.clone())
}

fn same_condition_outcome(activity: &SizedActivity, latest: &SizedActivity) -> bool {
    if activity_condition_key(activity) != activity_condition_key(latest) {
        return false;
    }
    match (&activity.outcome, &latest.outcome) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => activity.asset == latest.asset,
    }
}

fn opposite_condition_outcome(activity: &SizedActivity, latest: &SizedActivity) -> bool {
    if activity_condition_key(activity) != activity_condition_key(latest) {
        return false;
    }
    match (&activity.outcome, &latest.outcome) {
        (Some(left), Some(right)) => !left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

trait SaturatingSub {
    fn saturating_sub(self, rhs: Self) -> Self;
}

impl SaturatingSub for f64 {
    fn saturating_sub(self, rhs: Self) -> Self {
        let value = self - rhs;
        if value.is_sign_negative() { 0.0 } else { value }
    }
}

fn write_strategy_env(path: &Path, wallet: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(
        path,
        format!(
            "COPYTRADER_DISCOVERY_WALLET={wallet}\nCOPYTRADER_LEADER_WALLET={wallet}\nCOPYTRADER_SELECTED_FROM=minmax_activity_v1\n"
        ),
    )
    .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn read_selected_leader_wallet(path: &Path) -> Result<String, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    body.lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            match key.trim() {
                "COPYTRADER_DISCOVERY_WALLET" | "COPYTRADER_LEADER_WALLET" => {
                    let value = value.trim();
                    (!value.is_empty() && is_valid_evm_wallet(value)).then(|| value.to_string())
                }
                _ => None,
            }
        })
        .ok_or_else(|| format!("missing valid leader wallet in {}", path.display()))
}

fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
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

fn read_seen_txs(path: &Path) -> io::Result<BTreeSet<String>> {
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let body = fs::read_to_string(path)?;
    Ok(body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn write_seen_txs(path: &Path, seen: &BTreeSet<String>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut body = seen.iter().cloned().collect::<Vec<_>>().join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    fs::write(path, body)
}

fn write_report(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, lines.join("\n") + "\n")
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn persist_iteration_report(
    state_root: &Path,
    report_path: &Path,
    lines: &[String],
) -> Result<(), String> {
    write_report(report_path, lines)?;
    write_report(&state_root.join("latest.txt"), lines)?;
    write_status_json(&state_root.join("latest.json"), lines)
}

fn write_status_json(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, render_status_json(lines))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn render_status_json(lines: &[String]) -> String {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for line in lines.iter().rev() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !seen.insert(key.to_string()) {
            continue;
        }
        entries.push((key.to_string(), value.trim().to_string()));
    }
    entries.reverse();
    let body = entries
        .into_iter()
        .map(|(key, value)| format!("  \"{}\": {}", escape_json(&key), json_scalar(&key, &value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n{body}\n}}\n")
}

fn json_scalar(key: &str, value: &str) -> String {
    if key_forces_string(key) {
        return format!("\"{}\"", escape_json(value));
    }
    if let Ok(boolean) = value.parse::<bool>() {
        return boolean.to_string();
    }
    if let Ok(integer) = value.parse::<i64>() {
        return integer.to_string();
    }
    if let Ok(float) = value.parse::<f64>()
        && float.is_finite()
    {
        return if value.contains('.') {
            value.to_string()
        } else {
            float.to_string()
        };
    }
    format!("\"{}\"", escape_json(value))
}

fn key_forces_string(key: &str) -> bool {
    key.ends_with("_path")
        || key.contains("wallet")
        || key.contains("asset")
        || key.ends_with("_tx")
        || key.contains("slug")
        || key.ends_with("_side")
        || key.ends_with("_status")
        || matches!(
            key,
            "strategy"
                | "submit_mode"
                | "account_snapshot_refresh_output"
                | "post_submit_account_snapshot_refresh_output"
                | "report_path"
        )
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

fn now_nanos() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock error: {error}"))?
        .as_nanos())
}

fn current_unix_ms() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock error: {error}"))?
        .as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::{
        Options, compute_normalized_score, decide_condition_sizing, evaluate_sell_inventory,
        map_score_to_open_usdc, parse_args, run_minmax_follow, submit_output_contains_prefix,
        submit_output_contains_status, submit_output_marks_seen,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const WALLET: &str = "0x11084005d88A0840b5F38F8731CCa9152BbD99F7";

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-minmax-follow-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_normalized_strategy_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--user".into(),
            WALLET.into(),
            "--watch-limit".into(),
            "20".into(),
            "--loop-count".into(),
            "2".into(),
            "--min-open-usdc".into(),
            "2".into(),
            "--max-open-usdc".into(),
            "50".into(),
            "--forever".into(),
            "--flat-score".into(),
            "40".into(),
            "--max-total-exposure-usdc".into(),
            "100".into(),
            "--max-order-usdc".into(),
            "10".into(),
            "--allow-live-submit".into(),
        ])
        .expect("options parsed");

        assert_eq!(options.root, "..");
        assert_eq!(options.user.as_deref(), Some(WALLET));
        assert_eq!(options.watch_limit, 20);
        assert_eq!(options.loop_count, 2);
        assert!(options.forever);
        assert_eq!(options.min_open_usdc, 2.0);
        assert_eq!(options.max_open_usdc, 50.0);
        assert_eq!(options.flat_score, 40);
        assert_eq!(options.max_total_exposure_usdc.as_deref(), Some("100"));
        assert_eq!(options.max_order_usdc.as_deref(), Some("10"));
        assert!(options.allow_live_submit);
    }

    #[test]
    fn normalized_score_maps_range_and_flat_cases() {
        let activities = vec![
            super::SizedActivity {
                tx: "0x1".into(),
                timestamp: 1,
                side: "BUY".into(),
                asset: "asset-1".into(),
                condition_id: None,
                outcome: None,
                slug: None,
                size: 2.0,
                usdc_size: 1.0,
            },
            super::SizedActivity {
                tx: "0x2".into(),
                timestamp: 2,
                side: "BUY".into(),
                asset: "asset-1".into(),
                condition_id: None,
                outcome: None,
                slug: None,
                size: 20.0,
                usdc_size: 10.0,
            },
        ];

        assert_eq!(compute_normalized_score(&activities, 1.0, 50), 1);
        assert_eq!(compute_normalized_score(&activities, 10.0, 50), 100);
        assert_eq!(map_score_to_open_usdc(1, 1.0, 100.0), 1.0);
        assert_eq!(map_score_to_open_usdc(100, 1.0, 100.0), 100.0);

        let flat = vec![super::SizedActivity {
            usdc_size: 5.0,
            ..activities[0].clone()
        }];
        assert_eq!(compute_normalized_score(&flat, 5.0, 42), 42);
    }

    #[test]
    fn condition_decision_skips_small_unconfirmed_entry() {
        let activities = vec![super::SizedActivity {
            tx: "0xsmall".into(),
            timestamp: 20,
            side: "BUY".into(),
            asset: "asset-yes".into(),
            condition_id: Some("cond-1".into()),
            outcome: Some("Yes".into()),
            slug: Some("market-a".into()),
            size: 25.0,
            usdc_size: 3.5,
        }];

        let decision = decide_condition_sizing(&activities, &activities[0], 10.0, &Options::default());
        assert!(!decision.should_follow);
        assert_eq!(decision.decision_tag, "condition_unconfirmed_small_entry");
    }

    #[test]
    fn condition_decision_skips_small_opposite_hedge() {
        let activities = vec![
            super::SizedActivity {
                tx: "0xno".into(),
                timestamp: 10,
                side: "BUY".into(),
                asset: "asset-no".into(),
                condition_id: Some("cond-1".into()),
                outcome: Some("No".into()),
                slug: Some("market-a".into()),
                size: 1400.0,
                usdc_size: 200.0,
            },
            super::SizedActivity {
                tx: "0xyes".into(),
                timestamp: 20,
                side: "BUY".into(),
                asset: "asset-yes".into(),
                condition_id: Some("cond-1".into()),
                outcome: Some("Yes".into()),
                slug: Some("market-a".into()),
                size: 20.0,
                usdc_size: 2.5,
            },
        ];

        let decision = decide_condition_sizing(&activities, &activities[1], 10.0, &Options::default());
        assert!(!decision.should_follow);
        assert_eq!(decision.decision_tag, "condition_hedge_candidate");
        assert!(decision.opposite_outcome_sum_usdc > decision.same_outcome_sum_usdc);
    }

    #[test]
    fn sell_inventory_decision_skips_when_asset_not_held() {
        let root = unique_temp_dir("sell-no-inventory");
        fs::create_dir_all(root.join("runtime-verify-account")).expect("dir created");
        fs::write(
            root.join("runtime-verify-account/dashboard.json"),
            "{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}",
        )
        .expect("snapshot written");

        let latest = super::SizedActivity {
            tx: "0xsell".into(),
            timestamp: 20,
            side: "SELL".into(),
            asset: "asset-sell".into(),
            condition_id: Some("cond-1".into()),
            outcome: Some("Yes".into()),
            slug: Some("market-a".into()),
            size: 10.0,
            usdc_size: 5.0,
        };
        let options = Options {
            root: root.display().to_string(),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            ..Options::default()
        };

        let decision = evaluate_sell_inventory(&root, &options, &latest, true, 1.0).expect("decision");
        assert!(!decision.should_follow);
        assert_eq!(decision.decision_tag, "sell_without_inventory");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn sell_inventory_decision_caps_planned_sell_to_inventory() {
        let root = unique_temp_dir("sell-cap-inventory");
        fs::create_dir_all(root.join("runtime-verify-account")).expect("dir created");
        fs::write(
            root.join("runtime-verify-account/dashboard.json"),
            "{\"account_snapshot\":{\"positions\":[{\"asset_id\":\"asset-sell\",\"net_size\":\"1.0\",\"last_price\":\"0.5\",\"estimated_equity\":\"0.5\"}],\"open_orders\":[]}}",
        )
        .expect("snapshot written");

        let latest = super::SizedActivity {
            tx: "0xsell".into(),
            timestamp: 20,
            side: "SELL".into(),
            asset: "asset-sell".into(),
            condition_id: Some("cond-1".into()),
            outcome: Some("Yes".into()),
            slug: Some("market-a".into()),
            size: 10.0,
            usdc_size: 5.0,
        };
        let options = Options {
            root: root.display().to_string(),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            ..Options::default()
        };

        let decision = evaluate_sell_inventory(&root, &options, &latest, true, 1.0).expect("decision");
        assert!(decision.should_follow);
        assert_eq!(decision.decision_tag, "sell_inventory_capped");
        assert_eq!(decision.adjusted_open_usdc, 0.5);

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_scales_and_reports_submit_preview() {
        let root = unique_temp_dir("success");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":10,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":2.0,\"usdcSize\":1.0,\"transactionHash\":\"0xold\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}},\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );

        let submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &submit,
            "#!/bin/sh\nprintf 'live_submit_status=preview_only\\nnormalized_override='\"$8\"'\\n'\n",
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\ncat > \"$out\" <<'JSON'\n{\n  \"account_snapshot\": {\n    \"positions\": [],\n    \"open_orders\": []\n  }\n}\nJSON\nprintf 'account_monitor_output=%s\\n' \"$out\"\n",
        );
        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: false,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "normalized_score=100"));
        assert!(
            lines
                .iter()
                .any(|line| line == "normalized_open_usdc=100.000000")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "live_submit_status=preview_only")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "account_snapshot_refresh_status=ok")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("watch_user={WALLET}"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "poll_transport_mode=direct")
        );
        let latest =
            fs::read_to_string(root.join(format!(".omx/minmax-follow/{}/latest.txt", WALLET)))
                .expect("latest report exists");
        assert!(latest.contains("live_submit_status=preview_only"));
        assert!(latest.contains("poll_transport_mode=direct"));
        let latest_json =
            fs::read_to_string(root.join(format!(".omx/minmax-follow/{}/latest.json", WALLET)))
                .expect("latest json exists");
        assert!(latest_json.contains("\"live_submit_status\": \"preview_only\""));
        assert!(latest_json.contains("\"normalized_open_usdc\": 100.000000"));
        assert!(latest_json.contains("\"auto_submit_enabled\": false"));
        assert!(latest_json.contains("\"submit_mode\": \"preview\""));
        assert!(latest_json.contains("\"latest_asset\": \"asset-1\""));
        assert!(latest_json.contains("\"poll_transport_mode\": \"direct\""));
    }

    #[test]
    fn run_minmax_follow_skips_small_opposite_hedge_and_uses_latest_new_tx() {
        let root = unique_temp_dir("skip-hedge");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":10,\"type\":\"TRADE\",\"asset\":\"asset-no\",\"size\":1400.0,\"usdcSize\":200.0,\"transactionHash\":\"0xold\",\"price\":0.9,\"side\":\"BUY\",\"conditionId\":\"cond-1\",\"outcome\":\"No\",\"slug\":\"market-a\"}},\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-yes\",\"size\":20.0,\"usdcSize\":2.5,\"transactionHash\":\"0xhedge\",\"price\":0.1,\"side\":\"BUY\",\"conditionId\":\"cond-1\",\"outcome\":\"Yes\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xhedge\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=submitted\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "latest_tx=0xhedge"));
        assert!(lines.iter().any(|line| line == "condition_decision=condition_hedge_candidate"));
        assert!(lines.iter().any(|line| line == "submit_status=skipped_condition_hedge_candidate"));
        assert!(!forwarded.exists(), "submit should not be invoked for hedge candidate");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_skips_sell_when_inventory_missing() {
        let root = unique_temp_dir("skip-sell-no-inventory");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-sell\",\"size\":10.0,\"usdcSize\":5.0,\"transactionHash\":\"0xsell\",\"price\":0.5,\"side\":\"SELL\",\"conditionId\":\"cond-1\",\"outcome\":\"Yes\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xsell\\nlatest_new_side=SELL\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=submitted\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 0.1,
            max_open_usdc: 10.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "sell_inventory_decision=sell_without_inventory"));
        assert!(lines.iter().any(|line| line == "submit_status=skipped_sell_without_inventory"));
        assert!(!forwarded.exists(), "submit should not be invoked when no sell inventory");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_caps_sell_submit_to_inventory() {
        let root = unique_temp_dir("cap-sell-inventory");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-sell\",\"size\":10.0,\"usdcSize\":5.0,\"transactionHash\":\"0xsell\",\"price\":0.5,\"side\":\"SELL\",\"conditionId\":\"cond-1\",\"outcome\":\"Yes\",\"slug\":\"market-a\"}},\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":10,\"type\":\"TRADE\",\"asset\":\"asset-sell\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xbuy\",\"price\":0.5,\"side\":\"BUY\",\"conditionId\":\"cond-1\",\"outcome\":\"Yes\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xsell\\nlatest_new_side=SELL\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=preview_only\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[{\"asset_id\":\"asset-sell\",\"net_size\":\"0.5\",\"last_price\":\"0.5\",\"estimated_equity\":\"0.25\"}],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 0.1,
            max_open_usdc: 10.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "sell_inventory_decision=sell_inventory_capped"));
        assert!(lines.iter().any(|line| line == "planned_open_usdc=0.250000"));
        let args = fs::read_to_string(&forwarded).expect("forwarded args");
        assert!(args.contains("0.250000"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_passes_selected_latest_tx_file_to_submit() {
        let root = unique_temp_dir("selected-submit-activity");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":10,\"type\":\"TRADE\",\"asset\":\"asset-old\",\"size\":20.0,\"usdcSize\":2.0,\"transactionHash\":\"0xold\",\"price\":0.1,\"side\":\"BUY\",\"slug\":\"market-a\"}},\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-new\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.9,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-latest-activity.json");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nlatest=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--latest-activity\" ]; then latest=\"$2\"; shift 2; continue; fi\n  shift\n done\ncat \"$latest\" > '{forwarded}'\nprintf 'live_submit_status=preview_only\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: false,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "latest_tx=0xnew"));
        let selected = fs::read_to_string(&forwarded).expect("submit latest activity captured");
        assert!(selected.contains("0xnew"));
        assert!(!selected.contains("0xold"));
        assert!(selected.contains("asset-new"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_does_not_mark_seen_when_submit_reports_success_false() {
        let root = unique_temp_dir("submit-success-false-no-seen");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let submit_calls = root.join("submit-calls.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf 'called\\n' >> '{submit_calls}'\nprintf 'live_submit_status=submitted\\nsubmit_success=false\\n'\n",
                submit_calls = submit_calls.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 0.1,
            max_open_usdc: 10.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        run_minmax_follow(&options).expect("first run should succeed");
        run_minmax_follow(&options).expect("second run should succeed");

        let calls = fs::read_to_string(&submit_calls).expect("submit calls");
        assert_eq!(calls.lines().count(), 2);
        let seen_path = root.join(format!(".omx/minmax-follow/{WALLET}/submitted-tx.txt"));
        assert!(!seen_path.exists(), "seen file should not be written");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_does_not_forward_manual_gate_override_flags() {
        let root = unique_temp_dir("no-manual-gate-forward");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );

        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=preview_only\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: false,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: true,
            activity_under_budget: true,
            activity_capability_detected: true,
            positions_under_budget: true,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: None,
        };

        run_minmax_follow(&options).expect("strategy should succeed");
        let args = fs::read_to_string(&forwarded).expect("forwarded args exist");
        assert!(!args.contains("--activity-source-verified"));
        assert!(!args.contains("--activity-under-budget"));
        assert!(!args.contains("--activity-capability-detected"));
        assert!(!args.contains("--positions-under-budget"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn blocked_submit_output_does_not_mark_seen() {
        assert!(!submit_output_marks_seen(
            "live_submit_status=preview_only\n"
        ));
        assert!(submit_output_marks_seen("live_submit_status=submitted\n"));
        assert!(!submit_output_marks_seen(
            "live_submit_status=submitted\nsubmit_success=false\n"
        ));
        assert!(!submit_output_marks_seen(
            "live_gate_status=blocked:activity_source_over_budget\n"
        ));
        assert!(!submit_output_marks_seen(
            "live_submit_status=blocked:total_exposure_would_exceed_cap\n"
        ));
        assert!(submit_output_contains_status(
            "live_submit_status=submitted\n",
            "submitted"
        ));
        assert!(!submit_output_contains_status(
            "live_submit_status=preview_only\n",
            "submitted"
        ));
        assert!(submit_output_contains_prefix(
            "live_gate_status=blocked:activity_source_over_budget\n",
            "live_gate_status=blocked:"
        ));
    }

    #[test]
    fn run_minmax_follow_skips_submit_when_watch_reports_no_new_activity() {
        let root = unique_temp_dir("skip-no-new-activity");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xold\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=0\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=preview_only\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then out=\"$2\"; shift 2; continue; fi\n  shift\n done\nmkdir -p \"$(dirname \"$out\")\"\nprintf '{\"account_snapshot\":{\"positions\":[],\"open_orders\":[]}}' > \"$out\"\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "watch_has_new_activity=false"));
        assert!(lines.iter().any(|line| line == "account_snapshot_refresh_status=ok"));
        assert!(lines.iter().any(|line| line == "submit_status=skipped_no_new_activity"));
        assert!(!forwarded.exists(), "submit should not be invoked when no new activity");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_minmax_follow_skips_live_submit_when_account_snapshot_refresh_fails() {
        let root = unique_temp_dir("skip-refresh-failed");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            format!("COPYTRADER_DISCOVERY_WALLET={WALLET}\nCOPYTRADER_LEADER_WALLET={WALLET}\n"),
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/{wallet}'\ncat > '{root}/.omx/live-activity/{wallet}/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"{wallet}\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user={wallet}\\npoll_transport_mode=direct\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n",
                root = root.display(),
                wallet = WALLET
            ),
        );
        let submit = root.join("run_copytrader_live_submit_gate");
        let forwarded = root.join("forwarded-args.txt");
        write_executable(
            &submit,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{forwarded}'\nprintf 'live_submit_status=submitted\\n'\n",
                forwarded = forwarded.display()
            ),
        );
        let account_monitor = root.join("run_copytrader_account_monitor");
        write_executable(
            &account_monitor,
            "#!/bin/sh\nprintf 'refresh failed\\n' >&2\nexit 1\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            forever: false,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            max_total_exposure_usdc: Some("100".into()),
            max_order_usdc: Some("10".into()),
            account_snapshot: Some("runtime-verify-account/dashboard.json".into()),
            account_snapshot_max_age_secs: 300,
            activity_max_age_secs: 60,
            allow_live_submit: true,
            force_live_submit: false,
            ignore_seen_tx: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
            account_monitor_bin: Some(account_monitor.display().to_string()),
        };

        let lines = run_minmax_follow(&options).expect("strategy should succeed");
        assert!(lines.iter().any(|line| line == "account_snapshot_refresh_status=failed"));
        assert!(
            lines
                .iter()
                .any(|line| line == "submit_status=skipped_account_snapshot_refresh_failed")
        );
        assert!(!forwarded.exists(), "submit should not be invoked when refresh failed");

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
