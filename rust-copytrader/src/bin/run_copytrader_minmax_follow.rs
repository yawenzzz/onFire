use rust_copytrader::wallet_filter::{ActivityRecord, parse_activity_records};
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
    loop_interval_ms: u64,
    min_open_usdc: f64,
    max_open_usdc: f64,
    flat_score: u8,
    allow_live_submit: bool,
    activity_source_verified: bool,
    activity_under_budget: bool,
    activity_capability_detected: bool,
    positions_under_budget: bool,
    watch_bin: Option<String>,
    live_submit_bin: Option<String>,
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
            loop_interval_ms: 500,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            allow_live_submit: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: None,
            live_submit_bin: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SizedActivity {
    tx: String,
    timestamp: u64,
    side: String,
    asset: String,
    slug: Option<String>,
    size: f64,
    usdc_size: f64,
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
        "usage: run_copytrader_minmax_follow [--root <path>] [--user <wallet>] [--selected-leader-env <path>] [--proxy <url>] [--watch-limit <n>] [--loop-count <n>] [--loop-interval-ms <n>] [--min-open-usdc <decimal>] [--max-open-usdc <decimal>] [--flat-score <1..100>] [--allow-live-submit] [--activity-source-verified] [--activity-under-budget] [--activity-capability-detected] [--positions-under-budget] [--watch-bin <path>] [--live-submit-bin <path>]"
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
            "--allow-live-submit" => options.allow_live_submit = true,
            "--activity-source-verified" => options.activity_source_verified = true,
            "--activity-under-budget" => options.activity_under_budget = true,
            "--activity-capability-detected" => options.activity_capability_detected = true,
            "--positions-under-budget" => options.positions_under_budget = true,
            "--watch-bin" => options.watch_bin = Some(next_value(&mut iter, arg)?),
            "--live-submit-bin" => options.live_submit_bin = Some(next_value(&mut iter, arg)?),
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

    let mut last_lines = Vec::new();
    for index in 0..options.loop_count.max(1) {
        run_watch_once(&watch_bin, options, &wallet)?;
        let activities = read_recent_trade_activity(&latest_activity_path)?;
        let latest = latest_trade(&activities)?;
        let score =
            compute_normalized_score(&activities, latest.usdc_size.abs(), options.flat_score);
        let open_usdc = map_score_to_open_usdc(score, options.min_open_usdc, options.max_open_usdc);
        let history = history_min_max(&activities);
        let report_path = state_root.join(format!("run-{}-{}.txt", index, now_nanos()?));

        let mut lines = vec![
            "strategy=minmax_activity_v1".to_string(),
            format!("leader_wallet={wallet}"),
            format!("latest_activity_path={}", latest_activity_path.display()),
            format!("selected_leader_env_path={}", submit_selected_env.display()),
            format!("latest_tx={}", latest.tx),
            format!("latest_timestamp={}", latest.timestamp),
            format!("latest_side={}", latest.side),
            format!(
                "latest_slug={}",
                latest.slug.as_deref().unwrap_or("unknown")
            ),
            format!("latest_asset={}", latest.asset),
            format!("latest_activity_usdc={:.6}", latest.usdc_size),
            format!("recent_trade_count={}", activities.len()),
            format!("recent_usdc_min={:.6}", history.0),
            format!("recent_usdc_max={:.6}", history.1),
            format!("normalized_score={score}"),
            format!("normalized_open_usdc={open_usdc:.6}"),
        ];

        if seen.contains(&latest.tx) {
            lines.push("submit_status=skipped_duplicate_tx".to_string());
            lines.push(format!("report_path={}", report_path.display()));
            write_report(&report_path, &lines)?;
            last_lines = lines;
        } else {
            let submit_output = run_live_submit(
                &live_submit_bin,
                &root,
                &latest_activity_path,
                &submit_selected_env,
                open_usdc,
                options,
            )?;
            seen.insert(latest.tx.clone());
            write_seen_txs(&seen_path, &seen)
                .map_err(|error| format!("failed to write {}: {error}", seen_path.display()))?;
            lines.extend(
                submit_output
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToString::to_string),
            );
            lines.push(format!("report_path={}", report_path.display()));
            write_report(&report_path, &lines)?;
            last_lines = lines;
        }

        if index + 1 < options.loop_count {
            thread::sleep(Duration::from_millis(options.loop_interval_ms));
        }
    }
    Ok(last_lines)
}

fn run_watch_once(watch_bin: &Path, options: &Options, wallet: &str) -> Result<(), String> {
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
    let _ = run_command(watch_bin, &args, Some(Path::new(".")))?;
    Ok(())
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
    if options.activity_source_verified {
        args.push("--activity-source-verified".to_string());
    }
    if options.activity_under_budget {
        args.push("--activity-under-budget".to_string());
    }
    if options.activity_capability_detected {
        args.push("--activity-capability-detected".to_string());
    }
    if options.positions_under_budget {
        args.push("--positions-under-budget".to_string());
    }
    if options.allow_live_submit {
        args.push("--allow-live-submit".to_string());
    }
    let output = run_command(live_submit_bin, &args, Some(Path::new(".")))?;
    String::from_utf8(output.stdout)
        .map_err(|error| format!("run_copytrader_live_submit_gate stdout was not utf-8: {error}"))
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
                    (!value.is_empty()).then(|| value.to_string())
                }
                _ => None,
            }
        })
        .ok_or_else(|| format!("missing leader wallet in {}", path.display()))
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

fn now_nanos() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock error: {error}"))?
        .as_nanos())
}

#[cfg(test)]
mod tests {
    use super::{
        Options, compute_normalized_score, map_score_to_open_usdc, parse_args, run_minmax_follow,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            "0xleader".into(),
            "--watch-limit".into(),
            "20".into(),
            "--loop-count".into(),
            "2".into(),
            "--min-open-usdc".into(),
            "2".into(),
            "--max-open-usdc".into(),
            "50".into(),
            "--flat-score".into(),
            "40".into(),
            "--allow-live-submit".into(),
        ])
        .expect("options parsed");

        assert_eq!(options.root, "..");
        assert_eq!(options.user.as_deref(), Some("0xleader"));
        assert_eq!(options.watch_limit, 20);
        assert_eq!(options.loop_count, 2);
        assert_eq!(options.min_open_usdc, 2.0);
        assert_eq!(options.max_open_usdc, 50.0);
        assert_eq!(options.flat_score, 40);
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
                slug: None,
                size: 2.0,
                usdc_size: 1.0,
            },
            super::SizedActivity {
                tx: "0x2".into(),
                timestamp: 2,
                side: "BUY".into(),
                asset: "asset-1".into(),
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
    fn run_minmax_follow_scales_and_reports_submit_preview() {
        let root = unique_temp_dir("success");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let selected_env = root.join(".omx/discovery/selected-leader.env");
        fs::write(
            &selected_env,
            "COPYTRADER_DISCOVERY_WALLET=0xleader\nCOPYTRADER_LEADER_WALLET=0xleader\n",
        )
        .expect("env written");

        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            &format!(
                "#!/bin/sh\nmkdir -p '{root}/.omx/live-activity/0xleader'\ncat > '{root}/.omx/live-activity/0xleader/latest-activity.json' <<'JSON'\n[\n{{\"proxyWallet\":\"0xleader\",\"timestamp\":10,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":2.0,\"usdcSize\":1.0,\"transactionHash\":\"0xold\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}},\n{{\"proxyWallet\":\"0xleader\",\"timestamp\":20,\"type\":\"TRADE\",\"asset\":\"asset-1\",\"size\":20.0,\"usdcSize\":10.0,\"transactionHash\":\"0xnew\",\"price\":0.5,\"side\":\"BUY\",\"slug\":\"market-a\"}}\n]\nJSON\nprintf 'watch_user=0xleader\\npoll_new_events=1\\n'\n",
                root = root.display()
            ),
        );

        let submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &submit,
            "#!/bin/sh\nprintf 'live_submit_status=preview_only\\nnormalized_override='\"$8\"'\\n'\n",
        );

        let options = Options {
            root: root.display().to_string(),
            user: Some("0xleader".into()),
            selected_leader_env: Some(selected_env.display().to_string()),
            proxy: None,
            watch_limit: 10,
            loop_count: 1,
            loop_interval_ms: 0,
            min_open_usdc: 1.0,
            max_open_usdc: 100.0,
            flat_score: 50,
            allow_live_submit: false,
            activity_source_verified: false,
            activity_under_budget: false,
            activity_capability_detected: false,
            positions_under_budget: false,
            watch_bin: Some(watch.display().to_string()),
            live_submit_bin: Some(submit.display().to_string()),
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
    }
}
