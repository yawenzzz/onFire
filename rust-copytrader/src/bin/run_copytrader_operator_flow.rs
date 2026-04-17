use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    discovery_dir: String,
    leaderboard_base_url: Option<String>,
    activity_base_url: Option<String>,
    proxy: Option<String>,
    category: String,
    time_period: String,
    order_by: String,
    limit: usize,
    offset: usize,
    index: usize,
    activity_type: String,
    watch_poll_count: usize,
    watch_poll_interval_ms: u64,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    retry_count: usize,
    retry_delay_ms: u64,
    skip_activity: bool,
    skip_discovery: bool,
    skip_guarded_cycle: bool,
    position_targeting_demo: bool,
    live_submit_gate: bool,
    allow_live_submit: bool,
    discover_bin: Option<String>,
    watch_bin: Option<String>,
    guarded_bin: Option<String>,
    position_targeting_bin: Option<String>,
    live_submit_bin: Option<String>,
    operator_bin: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            discovery_dir: "../.omx/discovery".to_string(),
            leaderboard_base_url: None,
            activity_base_url: None,
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            category: "SPECIALIST".to_string(),
            time_period: "DAY".to_string(),
            order_by: "PNL".to_string(),
            limit: 20,
            offset: 0,
            index: 0,
            activity_type: "TRADE".to_string(),
            watch_poll_count: 0,
            watch_poll_interval_ms: 5_000,
            connect_timeout_ms: 5_000,
            max_time_ms: 12_000,
            retry_count: 1,
            retry_delay_ms: 500,
            skip_activity: false,
            skip_discovery: false,
            skip_guarded_cycle: false,
            position_targeting_demo: true,
            live_submit_gate: false,
            allow_live_submit: false,
            discover_bin: None,
            watch_bin: None,
            guarded_bin: None,
            position_targeting_bin: None,
            live_submit_bin: None,
            operator_bin: None,
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

    match run_operator_flow(&options) {
        Ok((report_path, None)) => {
            println!("operator_flow_report_path={}", report_path.display());
            ExitCode::SUCCESS
        }
        Ok((report_path, Some(error))) => {
            eprintln!("{error}");
            eprintln!("operator_flow_report_path={}", report_path.display());
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!(
        "usage: run_copytrader_operator_flow [--root <path>] [--discovery-dir <path>] [--leaderboard-base-url <url>] [--activity-base-url <url>] [--proxy <url>] [--category <value>] [--time-period <value>] [--order-by <value>] [--limit <n>] [--offset <n>] [--index <n>] [--activity-type <value>] [--watch-poll-count <n>] [--watch-poll-interval-ms <n>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--retry-count <n>] [--retry-delay-ms <n>] [--skip-activity] [--skip-discovery] [--skip-guarded-cycle] [--position-targeting-demo] [--skip-position-targeting-demo] [--live-submit-gate] [--allow-live-submit] [--discover-bin <path>] [--watch-bin <path>] [--guarded-bin <path>] [--position-targeting-bin <path>] [--live-submit-bin <path>] [--operator-bin <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--discovery-dir" => options.discovery_dir = next_value(&mut iter, arg)?,
            "--leaderboard-base-url" => {
                options.leaderboard_base_url = Some(next_value(&mut iter, arg)?)
            }
            "--activity-base-url" => options.activity_base_url = Some(next_value(&mut iter, arg)?),
            "--proxy" => options.proxy = Some(next_value(&mut iter, arg)?),
            "--category" => options.category = next_value(&mut iter, arg)?,
            "--time-period" => options.time_period = next_value(&mut iter, arg)?,
            "--order-by" => options.order_by = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--offset" => options.offset = parse_usize(&next_value(&mut iter, arg)?, "offset")?,
            "--index" => options.index = parse_usize(&next_value(&mut iter, arg)?, "index")?,
            "--activity-type" => options.activity_type = next_value(&mut iter, arg)?,
            "--watch-poll-count" => {
                options.watch_poll_count =
                    parse_usize(&next_value(&mut iter, arg)?, "watch-poll-count")?
            }
            "--watch-poll-interval-ms" => {
                options.watch_poll_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "watch-poll-interval-ms")?
            }
            "--connect-timeout-ms" => {
                options.connect_timeout_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "connect-timeout-ms")?
            }
            "--max-time-ms" => {
                options.max_time_ms = parse_u64(&next_value(&mut iter, arg)?, "max-time-ms")?
            }
            "--retry-count" => {
                options.retry_count = parse_usize(&next_value(&mut iter, arg)?, "retry-count")?
            }
            "--retry-delay-ms" => {
                options.retry_delay_ms = parse_u64(&next_value(&mut iter, arg)?, "retry-delay-ms")?
            }
            "--skip-activity" => options.skip_activity = true,
            "--skip-discovery" => options.skip_discovery = true,
            "--skip-guarded-cycle" => options.skip_guarded_cycle = true,
            "--position-targeting-demo" => options.position_targeting_demo = true,
            "--skip-position-targeting-demo" => options.position_targeting_demo = false,
            "--live-submit-gate" => options.live_submit_gate = true,
            "--allow-live-submit" => options.allow_live_submit = true,
            "--discover-bin" => options.discover_bin = Some(next_value(&mut iter, arg)?),
            "--watch-bin" => options.watch_bin = Some(next_value(&mut iter, arg)?),
            "--guarded-bin" => options.guarded_bin = Some(next_value(&mut iter, arg)?),
            "--position-targeting-bin" => {
                options.position_targeting_bin = Some(next_value(&mut iter, arg)?)
            }
            "--live-submit-bin" => options.live_submit_bin = Some(next_value(&mut iter, arg)?),
            "--operator-bin" => options.operator_bin = Some(next_value(&mut iter, arg)?),
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

fn run_operator_flow(options: &Options) -> Result<(PathBuf, Option<String>), String> {
    let root = PathBuf::from(&options.root);
    let operator_dir = root.join(".omx").join("operator-demo");
    fs::create_dir_all(&operator_dir)
        .map_err(|error| format!("failed to create {}: {error}", operator_dir.display()))?;
    let report_path = operator_report_path(&operator_dir)?;

    let discovery_result = if options.skip_discovery {
        Ok("skipped discovery step".to_string())
    } else {
        let discover_bin =
            resolve_bin_path("discover_copy_leader", options.discover_bin.as_deref())
                .map_err(|error| format!("failed to resolve discover_copy_leader: {error}"))?;
        run_command(
            &discover_bin,
            &build_discover_args(options),
            Some(Path::new(".")),
        )
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .map_err(|error| format!("discover_copy_leader stdout was not utf-8: {error}"))
        })
    };

    if discovery_result.is_ok() || options.skip_discovery {
        sync_selected_leader_env(&root, Path::new(&options.discovery_dir)).map_err(|error| {
            format!(
                "failed to sync selected leader env from {} into {}: {error}",
                options.discovery_dir,
                root.join(".omx/discovery/selected-leader.env").display()
            )
        })?;
    }
    let discovery_ready = discovery_result.is_ok() || options.skip_discovery;

    let watch_result = if !discovery_ready {
        Ok("skipped watcher step because discovery failed".to_string())
    } else if options.watch_poll_count == 0 {
        Ok("skipped watcher step".to_string())
    } else {
        let watch_bin =
            resolve_bin_path("watch_copy_leader_activity", options.watch_bin.as_deref()).map_err(
                |error| format!("failed to resolve watch_copy_leader_activity: {error}"),
            )?;
        run_command(&watch_bin, &build_watch_args(options), Some(Path::new("."))).and_then(
            |output| {
                String::from_utf8(output.stdout).map_err(|error| {
                    format!("watch_copy_leader_activity stdout was not utf-8: {error}")
                })
            },
        )
    };

    let guarded_result = if !discovery_ready {
        Ok("skipped guarded cycle because discovery failed".to_string())
    } else if options.skip_guarded_cycle || options.watch_poll_count == 0 {
        Ok("skipped guarded cycle".to_string())
    } else {
        let guarded_bin = resolve_bin_path(
            "run_copytrader_guarded_cycle",
            options.guarded_bin.as_deref(),
        )
        .map_err(|error| format!("failed to resolve run_copytrader_guarded_cycle: {error}"))?;
        run_command(
            &guarded_bin,
            &["--root".to_string(), options.root.clone()],
            Some(Path::new(".")),
        )
        .and_then(|output| {
            String::from_utf8(output.stdout).map_err(|error| {
                format!("run_copytrader_guarded_cycle stdout was not utf-8: {error}")
            })
        })
    };

    let position_targeting_result = if !discovery_ready {
        Ok("skipped position targeting because discovery failed".to_string())
    } else if !options.position_targeting_demo {
        Ok("skipped position targeting demo".to_string())
    } else {
        let position_targeting_bin = resolve_bin_path(
            "run_position_targeting_demo",
            options.position_targeting_bin.as_deref(),
        )
        .map_err(|error| format!("failed to resolve run_position_targeting_demo: {error}"))?;
        run_command(
            &position_targeting_bin,
            &["--root".to_string(), options.root.clone()],
            Some(Path::new(".")),
        )
        .and_then(|output| {
            String::from_utf8(output.stdout).map_err(|error| {
                format!("run_position_targeting_demo stdout was not utf-8: {error}")
            })
        })
    };

    let live_submit_result = if !discovery_ready {
        Ok("skipped live submit gate because discovery failed".to_string())
    } else if options.live_submit_gate
        && !options.skip_guarded_cycle
        && options.watch_poll_count > 0
    {
        let live_submit_bin = resolve_bin_path(
            "run_copytrader_live_submit_gate",
            options.live_submit_bin.as_deref(),
        )
        .map_err(|error| format!("failed to resolve run_copytrader_live_submit_gate: {error}"))?;
        run_command(
            &live_submit_bin,
            &build_live_submit_args(options),
            Some(Path::new(".")),
        )
        .and_then(|output| {
            String::from_utf8(output.stdout).map_err(|error| {
                format!("run_copytrader_live_submit_gate stdout was not utf-8: {error}")
            })
        })
    } else {
        Ok("skipped live submit gate".to_string())
    };

    let operator_result = if !discovery_ready {
        Ok("skipped operator demo because discovery failed".to_string())
    } else {
        resolve_bin_path("rust-copytrader", options.operator_bin.as_deref())
            .map_err(|error| format!("failed to resolve rust-copytrader operator binary: {error}"))
            .and_then(|operator_bin| {
                run_command(
                    &operator_bin,
                    &[
                        "--operator-demo".to_string(),
                        "--root".to_string(),
                        options.root.clone(),
                    ],
                    Some(Path::new(".")),
                )
            })
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .map_err(|error| format!("operator demo stdout was not utf-8: {error}"))
            })
    };

    let report = build_flow_report([
        (
            "discover_copy_leader",
            discovery_result.as_deref(),
            discovery_result.as_ref().err(),
        ),
        (
            "watch_copy_leader_activity",
            watch_result.as_deref(),
            watch_result.as_ref().err(),
        ),
        (
            "run_copytrader_guarded_cycle",
            guarded_result.as_deref(),
            guarded_result.as_ref().err(),
        ),
        (
            "run_position_targeting_demo",
            position_targeting_result.as_deref(),
            position_targeting_result.as_ref().err(),
        ),
        (
            "run_copytrader_live_submit_gate",
            live_submit_result.as_deref(),
            live_submit_result.as_ref().err(),
        ),
        (
            "operator_demo",
            operator_result.as_deref(),
            operator_result.as_ref().err(),
        ),
    ]);

    fs::write(&report_path, report)
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;
    let error = discovery_result
        .err()
        .or_else(|| watch_result.err())
        .or_else(|| guarded_result.err())
        .or_else(|| position_targeting_result.err())
        .or_else(|| live_submit_result.err())
        .or_else(|| operator_result.err());
    Ok((report_path, error))
}

fn build_flow_report(
    stages: [(&'static str, Result<&str, &String>, Option<&String>); 6],
) -> String {
    let mut report = stages
        .iter()
        .map(|(name, output, _)| {
            let section = match output {
                Ok(output) => output.trim_end().to_string(),
                Err(error) => format!("error={error}"),
            };
            format!("== {name} ==\n{section}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    if let Some((stage, _, Some(error))) = stages.iter().find(|(_, _, error)| error.is_some()) {
        report.push_str(&format!(
            "\nflow_failure_stage={stage}\nflow_failure_reason={error}"
        ));
    }

    report
}

fn operator_report_path(operator_dir: &Path) -> Result<PathBuf, String> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time error: {error}"))?
        .as_nanos();
    Ok(operator_dir.join(format!("discover-and-demo-{run_id}.txt")))
}

fn sync_selected_leader_env(root: &Path, discovery_dir: &Path) -> io::Result<()> {
    let source = discovery_dir.join("selected-leader.env");
    if !source.exists() {
        return Ok(());
    }

    let target = root.join(".omx/discovery/selected-leader.env");
    if source == target {
        return Ok(());
    }

    let bytes = fs::read(&source)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, bytes)
}

fn build_discover_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--discovery-dir".to_string(),
        options.discovery_dir.clone(),
        "--category".to_string(),
        options.category.clone(),
        "--time-period".to_string(),
        options.time_period.clone(),
        "--order-by".to_string(),
        options.order_by.clone(),
        "--limit".to_string(),
        options.limit.to_string(),
        "--offset".to_string(),
        options.offset.to_string(),
        "--index".to_string(),
        options.index.to_string(),
        "--activity-type".to_string(),
        options.activity_type.clone(),
        "--connect-timeout-ms".to_string(),
        options.connect_timeout_ms.to_string(),
        "--max-time-ms".to_string(),
        options.max_time_ms.to_string(),
        "--retry-count".to_string(),
        options.retry_count.to_string(),
        "--retry-delay-ms".to_string(),
        options.retry_delay_ms.to_string(),
    ];
    if let Some(base_url) = &options.leaderboard_base_url {
        args.push("--leaderboard-base-url".to_string());
        args.push(base_url.clone());
    }
    if let Some(base_url) = &options.activity_base_url {
        args.push("--activity-base-url".to_string());
        args.push(base_url.clone());
    }
    if let Some(proxy) = &options.proxy {
        args.push("--proxy".to_string());
        args.push(proxy.clone());
    }
    if options.skip_activity {
        args.push("--skip-activity".to_string());
    }
    args
}

fn build_watch_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--root".to_string(),
        options.root.clone(),
        "--poll-count".to_string(),
        options.watch_poll_count.to_string(),
        "--poll-interval-ms".to_string(),
        options.watch_poll_interval_ms.to_string(),
        "--activity-type".to_string(),
        options.activity_type.clone(),
        "--connect-timeout-ms".to_string(),
        options.connect_timeout_ms.to_string(),
        "--max-time-ms".to_string(),
        options.max_time_ms.to_string(),
        "--retry-count".to_string(),
        options.retry_count.to_string(),
        "--retry-delay-ms".to_string(),
        options.retry_delay_ms.to_string(),
    ];
    if let Some(base_url) = &options.activity_base_url {
        args.push("--base-url".to_string());
        args.push(base_url.clone());
    }
    if let Some(proxy) = &options.proxy {
        args.push("--proxy".to_string());
        args.push(proxy.clone());
    }
    args
}

fn build_live_submit_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--root".to_string(),
        options.root.clone(),
        "--activity-source-verified".to_string(),
        "--activity-under-budget".to_string(),
        "--activity-capability-detected".to_string(),
        "--positions-under-budget".to_string(),
    ];
    if options.allow_live_submit {
        args.push("--allow-live-submit".to_string());
    }
    args
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
    use super::{
        build_discover_args, build_live_submit_args, build_watch_args, parse_args,
        run_operator_flow, sync_selected_leader_env,
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
        std::env::temp_dir().join(format!("run-copytrader-operator-flow-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_root_and_skip_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--discovery-dir".into(),
            "../.omx/discovery".into(),
            "--leaderboard-base-url".into(),
            "https://example.com/leaderboard".into(),
            "--activity-base-url".into(),
            "https://example.com/activity".into(),
            "--watch-poll-count".into(),
            "2".into(),
            "--watch-poll-interval-ms".into(),
            "10".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
            "--retry-count".into(),
            "2".into(),
            "--retry-delay-ms".into(),
            "25".into(),
            "--skip-activity".into(),
            "--skip-discovery".into(),
            "--skip-guarded-cycle".into(),
            "--position-targeting-demo".into(),
            "--live-submit-gate".into(),
            "--allow-live-submit".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert_eq!(options.discovery_dir, "../.omx/discovery");
        assert_eq!(
            options.leaderboard_base_url.as_deref(),
            Some("https://example.com/leaderboard")
        );
        assert_eq!(
            options.activity_base_url.as_deref(),
            Some("https://example.com/activity")
        );
        assert_eq!(options.watch_poll_count, 2);
        assert_eq!(options.watch_poll_interval_ms, 10);
        assert_eq!(options.proxy.as_deref(), Some("http://127.0.0.1:7897"));
        assert_eq!(options.retry_count, 2);
        assert_eq!(options.retry_delay_ms, 25);
        assert!(options.skip_activity);
        assert!(options.skip_discovery);
        assert!(options.skip_guarded_cycle);
        assert!(options.position_targeting_demo);
        assert!(options.live_submit_gate);
        assert!(options.allow_live_submit);
    }

    #[test]
    fn build_discover_args_keeps_current_timeout_and_selection_defaults() {
        let options = parse_args(&[
            "--limit".into(),
            "5".into(),
            "--index".into(),
            "2".into(),
            "--activity-type".into(),
            "TRADE".into(),
        ])
        .expect("parse");

        let args = build_discover_args(&options);

        assert!(args.contains(&"--limit".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"--index".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"--connect-timeout-ms".to_string()));
        assert!(args.contains(&"5000".to_string()));
        assert!(args.contains(&"--retry-count".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(!args.contains(&"--proxy".to_string()));
    }

    #[test]
    fn build_watch_args_includes_proxy_and_poll_config() {
        let options = parse_args(&[
            "--watch-poll-count".into(),
            "3".into(),
            "--watch-poll-interval-ms".into(),
            "25".into(),
            "--activity-base-url".into(),
            "https://example.com/activity".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
            "--retry-count".into(),
            "2".into(),
            "--retry-delay-ms".into(),
            "25".into(),
        ])
        .expect("parse");

        let args = build_watch_args(&options);

        assert!(args.contains(&"--poll-count".to_string()));
        assert!(args.contains(&"3".to_string()));
        assert!(args.contains(&"--poll-interval-ms".to_string()));
        assert!(args.contains(&"25".to_string()));
        assert!(args.contains(&"--base-url".to_string()));
        assert!(args.contains(&"https://example.com/activity".to_string()));
        assert!(args.contains(&"--proxy".to_string()));
        assert!(args.contains(&"--retry-count".to_string()));
        assert!(args.contains(&"2".to_string()));
    }

    #[test]
    fn build_live_submit_args_carries_gate_unlock_flags() {
        let options = parse_args(&["--allow-live-submit".into()]).expect("parse");
        let args = build_live_submit_args(&options);

        assert!(args.contains(&"--activity-source-verified".to_string()));
        assert!(args.contains(&"--positions-under-budget".to_string()));
        assert!(args.contains(&"--allow-live-submit".to_string()));
    }

    #[test]
    fn run_operator_flow_combines_discovery_and_operator_reports() {
        let root = unique_temp_dir("flow");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\nselected_leader_env_path=../.omx/discovery/selected-leader.env\\n'\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\nselected_leader_wallet=0xleader\\n'\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\nprintf 'watch_user=0xleader\\npoll_new_events=1\\n'\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\nprintf 'mode=guarded-cycle\\nlast_submit_status=verified\\n'\n",
        );
        let position_targeting = root.join("run_position_targeting_demo");
        write_executable(
            &position_targeting,
            "#!/usr/bin/env bash\nprintf 'mode=position-targeting-demo\\ntarget_count=2\\ndelta_count=1\\n'\n",
        );
        let live_submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &live_submit,
            "#!/usr/bin/env bash\nprintf 'mode=live-submit-gate\\nlive_submit_status=preview_only\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discovery-dir".into(),
            root.join(".omx/discovery").display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--watch-poll-count".into(),
            "1".into(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--position-targeting-bin".into(),
            position_targeting.display().to_string(),
            "--position-targeting-demo".into(),
            "--live-submit-bin".into(),
            live_submit.display().to_string(),
            "--live-submit-gate".into(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should succeed");
        assert!(error.is_none());
        let report = fs::read_to_string(&report_path).expect("report exists");

        assert!(report.contains("== discover_copy_leader =="));
        assert!(report.contains("selected_wallet=0xleader"));
        assert!(report.contains("== watch_copy_leader_activity =="));
        assert!(report.contains("watch_user=0xleader"));
        assert!(report.contains("== run_copytrader_guarded_cycle =="));
        assert!(report.contains("mode=guarded-cycle"));
        assert!(report.contains("== run_position_targeting_demo =="));
        assert!(report.contains("mode=position-targeting-demo"));
        assert!(report.contains("== run_copytrader_live_submit_gate =="));
        assert!(report.contains("live_submit_status=preview_only"));
        assert!(report.contains("== operator_demo =="));
        assert!(report.contains("mode=operator-demo"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn sync_selected_leader_env_copies_custom_discovery_env_into_root_scope() {
        let root = unique_temp_dir("sync-env");
        let custom_discovery = root.join("custom-discovery");
        fs::create_dir_all(&custom_discovery).expect("custom discovery created");
        fs::write(
            custom_discovery.join("selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xleader-sync\n",
        )
        .expect("selected leader written");

        sync_selected_leader_env(&root, &custom_discovery).expect("sync should succeed");

        let synced = fs::read_to_string(root.join(".omx/discovery/selected-leader.env"))
            .expect("synced env exists");
        assert!(synced.contains("COPYTRADER_DISCOVERY_WALLET=0xleader-sync"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_can_skip_discovery() {
        let root = unique_temp_dir("skip");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--skip-discovery".into(),
            "--skip-position-targeting-demo".into(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should succeed");
        assert!(error.is_none());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("skipped discovery step"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_persists_failure_report_when_discovery_fails() {
        let root = unique_temp_dir("failure");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\necho 'discovery failed' >&2\nexit 1\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\necho 'watch failed' >&2\nexit 1\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--watch-poll-count".into(),
            "1".into(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=discover_copy_leader"));
        assert!(report.contains("discover_copy_leader"));
        assert!(report.contains("skipped watcher step because discovery failed"));
        assert!(report.contains("skipped position targeting because discovery failed"));
        assert!(report.contains("skipped operator demo because discovery failed"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_persists_failure_report_when_watcher_fails() {
        let root = unique_temp_dir("watch-failure");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\nselected_leader_env_path=../.omx/discovery/selected-leader.env\\n'\nmkdir -p ../.omx/discovery\nprintf 'COPYTRADER_DISCOVERY_WALLET=0xleader\\n' > ../.omx/discovery/selected-leader.env\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\necho 'watch failed' >&2\nexit 1\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\nprintf 'mode=guarded-cycle\\n'\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--watch-poll-count".into(),
            "1".into(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=watch_copy_leader_activity"));
        assert!(report.contains("watch_copy_leader_activity"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_persists_failure_report_when_guarded_cycle_fails() {
        let root = unique_temp_dir("guarded-failure");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\nselected_leader_env_path=../.omx/discovery/selected-leader.env\\n'\nmkdir -p ../.omx/discovery\nprintf 'COPYTRADER_DISCOVERY_WALLET=0xleader\\n' > ../.omx/discovery/selected-leader.env\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\nprintf 'watch_user=0xleader\\n'\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\necho 'guarded failed' >&2\nexit 1\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--watch-poll-count".into(),
            "1".into(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=run_copytrader_guarded_cycle"));
        assert!(report.contains("run_copytrader_guarded_cycle"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_persists_failure_report_when_live_submit_gate_fails() {
        let root = unique_temp_dir("live-submit-failure");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\nselected_leader_env_path=../.omx/discovery/selected-leader.env\\n'\nmkdir -p ../.omx/discovery\nprintf 'COPYTRADER_DISCOVERY_WALLET=0xleader\\n' > ../.omx/discovery/selected-leader.env\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\nprintf 'watch_user=0xleader\\n'\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\nprintf 'mode=guarded-cycle\\nlast_submit_status=verified\\n'\n",
        );
        let live_submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &live_submit,
            "#!/usr/bin/env bash\necho 'live submit failed' >&2\nexit 1\n",
        );
        let position_targeting = root.join("run_position_targeting_demo");
        write_executable(
            &position_targeting,
            "#!/usr/bin/env bash\nprintf 'mode=position-targeting-demo\\ntarget_count=2\\n'\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--watch-poll-count".into(),
            "1".into(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--position-targeting-bin".into(),
            position_targeting.display().to_string(),
            "--live-submit-bin".into(),
            live_submit.display().to_string(),
            "--live-submit-gate".into(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=run_copytrader_live_submit_gate"));
        assert!(report.contains("run_copytrader_live_submit_gate"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn run_operator_flow_persists_failure_report_when_position_targeting_demo_fails() {
        let root = unique_temp_dir("position-targeting-failure");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\nselected_leader_env_path=../.omx/discovery/selected-leader.env\\n'\nmkdir -p ../.omx/discovery\nprintf 'COPYTRADER_DISCOVERY_WALLET=0xleader\\n' > ../.omx/discovery/selected-leader.env\n",
        );
        let position_targeting = root.join("run_position_targeting_demo");
        write_executable(
            &position_targeting,
            "#!/usr/bin/env bash\necho 'position targeting failed' >&2\nexit 1\n",
        );
        let operator = root.join("rust-copytrader");
        write_executable(
            &operator,
            "#!/usr/bin/env bash\nprintf 'mode=operator-demo\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--position-targeting-bin".into(),
            position_targeting.display().to_string(),
            "--position-targeting-demo".into(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=run_position_targeting_demo"));
        assert!(report.contains("run_position_targeting_demo"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
