use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    discovery_dir: String,
    proxy: Option<String>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    retry_count: usize,
    retry_delay_ms: u64,
    loop_count: usize,
    loop_interval_ms: u64,
    watch_poll_count: usize,
    live_submit_gate: bool,
    allow_live_submit: bool,
    discover_bin: Option<String>,
    watch_bin: Option<String>,
    guarded_bin: Option<String>,
    live_submit_bin: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            discovery_dir: "../.omx/discovery".to_string(),
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            connect_timeout_ms: 8_000,
            max_time_ms: 20_000,
            retry_count: 1,
            retry_delay_ms: 500,
            loop_count: 1,
            loop_interval_ms: 5_000,
            watch_poll_count: 1,
            live_submit_gate: false,
            allow_live_submit: false,
            discover_bin: None,
            watch_bin: None,
            guarded_bin: None,
            live_submit_bin: None,
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

    match run_auto_guarded_loop(&options) {
        Ok((report_path, None)) => {
            println!("auto_guarded_report_path={}", report_path.display());
            ExitCode::SUCCESS
        }
        Ok((report_path, Some(error))) => {
            eprintln!("{error}");
            eprintln!("auto_guarded_report_path={}", report_path.display());
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
        "usage: run_copytrader_auto_guarded_loop [--root <path>] [--discovery-dir <path>] [--proxy <url>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--retry-count <n>] [--retry-delay-ms <n>] [--loop-count <n>] [--loop-interval-ms <n>] [--watch-poll-count <n>] [--live-submit-gate] [--allow-live-submit] [--discover-bin <path>] [--watch-bin <path>] [--guarded-bin <path>] [--live-submit-bin <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--discovery-dir" => options.discovery_dir = next_value(&mut iter, arg)?,
            "--proxy" => options.proxy = Some(next_value(&mut iter, arg)?),
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
            "--loop-count" => {
                options.loop_count = parse_usize(&next_value(&mut iter, arg)?, "loop-count")?
            }
            "--loop-interval-ms" => {
                options.loop_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "loop-interval-ms")?
            }
            "--watch-poll-count" => {
                options.watch_poll_count =
                    parse_usize(&next_value(&mut iter, arg)?, "watch-poll-count")?
            }
            "--live-submit-gate" => options.live_submit_gate = true,
            "--allow-live-submit" => options.allow_live_submit = true,
            "--discover-bin" => options.discover_bin = Some(next_value(&mut iter, arg)?),
            "--watch-bin" => options.watch_bin = Some(next_value(&mut iter, arg)?),
            "--guarded-bin" => options.guarded_bin = Some(next_value(&mut iter, arg)?),
            "--live-submit-bin" => options.live_submit_bin = Some(next_value(&mut iter, arg)?),
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

fn run_auto_guarded_loop(options: &Options) -> Result<(PathBuf, Option<String>), String> {
    let root = PathBuf::from(&options.root);
    let report_dir = root.join(".omx").join("auto-guarded");
    fs::create_dir_all(&report_dir)
        .map_err(|error| format!("failed to create {}: {error}", report_dir.display()))?;

    let discover_bin = resolve_bin_path("discover_copy_leader", options.discover_bin.as_deref())
        .map_err(|error| format!("failed to resolve discover_copy_leader: {error}"))?;
    let watch_bin = resolve_bin_path("watch_copy_leader_activity", options.watch_bin.as_deref())
        .map_err(|error| format!("failed to resolve watch_copy_leader_activity: {error}"))?;
    let guarded_bin = resolve_bin_path(
        "run_copytrader_guarded_cycle",
        options.guarded_bin.as_deref(),
    )
    .map_err(|error| format!("failed to resolve run_copytrader_guarded_cycle: {error}"))?;
    let live_submit_bin = resolve_bin_path(
        "run_copytrader_live_submit_gate",
        options.live_submit_bin.as_deref(),
    )
    .map_err(|error| format!("failed to resolve run_copytrader_live_submit_gate: {error}"))?;

    let report_path = report_dir.join(format!("auto-guarded-{}.txt", now_nanos()?));
    let mut report_sections = Vec::new();
    let mut failure: Option<(String, String)> = None;

    for iteration in 0..options.loop_count.max(1) {
        let discover_output = run_command(
            &discover_bin,
            &build_discover_args(options),
            Some(Path::new(".")),
        );
        let discover_text = match &discover_output {
            Ok(output) => decode_stdout("discover_copy_leader", output)?,
            Err(error) => {
                failure = Some(("discover_copy_leader".to_string(), error.clone()));
                format!("error={error}")
            }
        };
        report_sections.push(format!(
            "== iteration {iteration} / discover_copy_leader ==\n{}",
            discover_text.trim_end()
        ));
        if failure.is_some() {
            break;
        }

        let watch_output = run_command(
            &watch_bin,
            &build_watch_args(options),
            Some(Path::new(".")),
        );
        let watch_text = match &watch_output {
            Ok(output) => decode_stdout("watch_copy_leader_activity", output)?,
            Err(error) => {
                failure = Some(("watch_copy_leader_activity".to_string(), error.clone()));
                format!("error={error}")
            }
        };
        report_sections.push(format!(
            "== iteration {iteration} / watch_copy_leader_activity ==\n{}",
            watch_text.trim_end()
        ));
        if failure.is_some() {
            break;
        }

        let guarded_output = run_command(
            &guarded_bin,
            &[
                "--root".to_string(),
                options.root.clone(),
            ],
            Some(Path::new(".")),
        );
        let guarded_text = match &guarded_output {
            Ok(output) => decode_stdout("run_copytrader_guarded_cycle", output)?,
            Err(error) => {
                failure = Some(("run_copytrader_guarded_cycle".to_string(), error.clone()));
                format!("error={error}")
            }
        };
        report_sections.push(format!(
            "== iteration {iteration} / run_copytrader_guarded_cycle ==\n{}",
            guarded_text.trim_end()
        ));
        if failure.is_some() {
            break;
        }

        if options.live_submit_gate {
            let live_submit_output = run_command(
                &live_submit_bin,
                &build_live_submit_args(options),
                Some(Path::new(".")),
            );
            let live_submit_text = match &live_submit_output {
                Ok(output) => decode_stdout("run_copytrader_live_submit_gate", output)?,
                Err(error) => {
                    failure = Some(("run_copytrader_live_submit_gate".to_string(), error.clone()));
                    format!("error={error}")
                }
            };
            report_sections.push(format!(
                "== iteration {iteration} / run_copytrader_live_submit_gate ==\n{}",
                live_submit_text.trim_end()
            ));
            if failure.is_some() {
                break;
            }
        }

        if iteration + 1 < options.loop_count {
            thread::sleep(Duration::from_millis(options.loop_interval_ms));
        }
    }

    if let Some((stage, error)) = &failure {
        report_sections.push(format!("flow_failure_stage={stage}\nflow_failure_reason={error}"));
    }

    fs::write(&report_path, report_sections.join("\n"))
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;

    Ok((report_path, failure.map(|(_, error)| error)))
}

fn build_discover_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--discovery-dir".to_string(),
        options.discovery_dir.clone(),
        "--connect-timeout-ms".to_string(),
        options.connect_timeout_ms.to_string(),
        "--max-time-ms".to_string(),
        options.max_time_ms.to_string(),
        "--retry-count".to_string(),
        options.retry_count.to_string(),
        "--retry-delay-ms".to_string(),
        options.retry_delay_ms.to_string(),
    ];
    if let Some(proxy) = &options.proxy {
        args.push("--proxy".to_string());
        args.push(proxy.clone());
    }
    args
}

fn build_watch_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--root".to_string(),
        options.root.clone(),
        "--poll-count".to_string(),
        options.watch_poll_count.to_string(),
        "--connect-timeout-ms".to_string(),
        options.connect_timeout_ms.to_string(),
        "--max-time-ms".to_string(),
        options.max_time_ms.to_string(),
        "--retry-count".to_string(),
        options.retry_count.to_string(),
        "--retry-delay-ms".to_string(),
        options.retry_delay_ms.to_string(),
    ];
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

fn decode_stdout(stage: &str, output: &Output) -> Result<String, String> {
    String::from_utf8(output.stdout.clone())
        .map_err(|error| format!("{stage} stdout was not utf-8: {error}"))
}

fn now_nanos() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time error: {error}"))
        .map(|duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::{
        build_discover_args, build_live_submit_args, build_watch_args, parse_args,
        run_auto_guarded_loop,
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
        std::env::temp_dir().join(format!("run-copytrader-auto-guarded-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_loop_and_proxy_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--loop-count".into(),
            "2".into(),
            "--loop-interval-ms".into(),
            "10".into(),
            "--watch-poll-count".into(),
            "3".into(),
            "--live-submit-gate".into(),
            "--allow-live-submit".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
        ])
        .expect("parse");

        assert_eq!(options.loop_count, 2);
        assert_eq!(options.loop_interval_ms, 10);
        assert_eq!(options.watch_poll_count, 3);
        assert!(options.live_submit_gate);
        assert!(options.allow_live_submit);
        assert_eq!(options.proxy.as_deref(), Some("http://127.0.0.1:7897"));
    }

    #[test]
    fn build_command_args_forward_proxy_and_retry_settings() {
        let options = parse_args(&[
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
            "--retry-count".into(),
            "2".into(),
            "--retry-delay-ms".into(),
            "25".into(),
            "--watch-poll-count".into(),
            "4".into(),
        ])
        .expect("parse");

        let discover_args = build_discover_args(&options);
        let watch_args = build_watch_args(&options);

        assert!(discover_args.contains(&"--proxy".to_string()));
        assert!(discover_args.contains(&"http://127.0.0.1:7897".to_string()));
        assert!(watch_args.contains(&"--poll-count".to_string()));
        assert!(watch_args.contains(&"4".to_string()));
        assert!(watch_args.contains(&"--retry-count".to_string()));
        assert!(watch_args.contains(&"2".to_string()));
    }

    #[test]
    fn build_live_submit_args_exposes_gate_flags() {
        let options = parse_args(&["--allow-live-submit".into()]).expect("parse");
        let args = build_live_submit_args(&options);

        assert!(args.contains(&"--activity-source-verified".to_string()));
        assert!(args.contains(&"--positions-under-budget".to_string()));
        assert!(args.contains(&"--allow-live-submit".to_string()));
    }

    #[test]
    fn auto_guarded_loop_combines_all_stage_outputs() {
        let root = unique_temp_dir("success");
        fs::create_dir_all(root.join(".omx/auto-guarded")).expect("auto dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\n'\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\nprintf 'watch_user=0xleader\\npoll_new_events=1\\n'\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\nprintf 'mode=guarded-cycle\\ncycle_outcome=processed\\n'\n",
        );
        let live_submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &live_submit,
            "#!/usr/bin/env bash\nprintf 'mode=live-submit-gate\\nlive_submit_status=preview_only\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--live-submit-bin".into(),
            live_submit.display().to_string(),
            "--live-submit-gate".into(),
            "--loop-count".into(),
            "1".into(),
            "--watch-poll-count".into(),
            "1".into(),
        ])
        .expect("parse");

        let (report_path, error) = run_auto_guarded_loop(&options).expect("loop should succeed");
        assert!(error.is_none());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("== iteration 0 / discover_copy_leader =="));
        assert!(report.contains("selected_wallet=0xleader"));
        assert!(report.contains("== iteration 0 / watch_copy_leader_activity =="));
        assert!(report.contains("watch_user=0xleader"));
        assert!(report.contains("== iteration 0 / run_copytrader_guarded_cycle =="));
        assert!(report.contains("cycle_outcome=processed"));
        assert!(report.contains("== iteration 0 / run_copytrader_live_submit_gate =="));
        assert!(report.contains("live_submit_status=preview_only"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn auto_guarded_loop_persists_failure_stage() {
        let root = unique_temp_dir("failure");
        fs::create_dir_all(root.join(".omx/auto-guarded")).expect("auto dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\n'\n",
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
        let live_submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &live_submit,
            "#!/usr/bin/env bash\nprintf 'mode=live-submit-gate\\n'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--live-submit-bin".into(),
            live_submit.display().to_string(),
            "--live-submit-gate".into(),
            "--loop-count".into(),
            "1".into(),
            "--watch-poll-count".into(),
            "1".into(),
        ])
        .expect("parse");

        let (report_path, error) = run_auto_guarded_loop(&options).expect("loop returns report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=watch_copy_leader_activity"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn auto_guarded_loop_persists_failure_stage_when_live_submit_gate_fails() {
        let root = unique_temp_dir("live-submit-failure");
        fs::create_dir_all(root.join(".omx/auto-guarded")).expect("auto dir created");

        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            "#!/usr/bin/env bash\nprintf 'selected_wallet=0xleader\\n'\n",
        );
        let watch = root.join("watch_copy_leader_activity");
        write_executable(
            &watch,
            "#!/usr/bin/env bash\nprintf 'watch_user=0xleader\\npoll_new_events=1\\n'\n",
        );
        let guarded = root.join("run_copytrader_guarded_cycle");
        write_executable(
            &guarded,
            "#!/usr/bin/env bash\nprintf 'mode=guarded-cycle\\ncycle_outcome=processed\\n'\n",
        );
        let live_submit = root.join("run_copytrader_live_submit_gate");
        write_executable(
            &live_submit,
            "#!/usr/bin/env bash\necho 'live submit failed' >&2\nexit 1\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--watch-bin".into(),
            watch.display().to_string(),
            "--guarded-bin".into(),
            guarded.display().to_string(),
            "--live-submit-bin".into(),
            live_submit.display().to_string(),
            "--live-submit-gate".into(),
            "--loop-count".into(),
            "1".into(),
            "--watch-poll-count".into(),
            "1".into(),
        ])
        .expect("parse");

        let (report_path, error) = run_auto_guarded_loop(&options).expect("loop returns report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=run_copytrader_live_submit_gate"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
