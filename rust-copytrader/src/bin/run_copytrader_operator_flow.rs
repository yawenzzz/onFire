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
    connect_timeout_ms: u64,
    max_time_ms: u64,
    skip_activity: bool,
    skip_discovery: bool,
    discover_bin: Option<String>,
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
            category: "OVERALL".to_string(),
            time_period: "DAY".to_string(),
            order_by: "PNL".to_string(),
            limit: 20,
            offset: 0,
            index: 0,
            activity_type: "TRADE".to_string(),
            connect_timeout_ms: 5_000,
            max_time_ms: 12_000,
            skip_activity: false,
            skip_discovery: false,
            discover_bin: None,
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
        "usage: run_copytrader_operator_flow [--root <path>] [--discovery-dir <path>] [--leaderboard-base-url <url>] [--activity-base-url <url>] [--proxy <url>] [--category <value>] [--time-period <value>] [--order-by <value>] [--limit <n>] [--offset <n>] [--index <n>] [--activity-type <value>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--skip-activity] [--skip-discovery] [--discover-bin <path>] [--operator-bin <path>]"
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
            "--connect-timeout-ms" => {
                options.connect_timeout_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "connect-timeout-ms")?
            }
            "--max-time-ms" => {
                options.max_time_ms = parse_u64(&next_value(&mut iter, arg)?, "max-time-ms")?
            }
            "--skip-activity" => options.skip_activity = true,
            "--skip-discovery" => options.skip_discovery = true,
            "--discover-bin" => options.discover_bin = Some(next_value(&mut iter, arg)?),
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

    let operator_result = resolve_bin_path("rust-copytrader", options.operator_bin.as_deref())
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
        });

    let report = build_flow_report(
        discovery_result.as_deref(),
        discovery_result.as_ref().err(),
        operator_result.as_deref(),
        operator_result.as_ref().err(),
    );

    fs::write(&report_path, report)
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;
    let error = discovery_result.err().or_else(|| operator_result.err());
    Ok((report_path, error))
}

fn build_flow_report(
    discovery_output: Result<&str, &String>,
    discovery_error: Option<&String>,
    operator_output: Result<&str, &String>,
    operator_error: Option<&String>,
) -> String {
    let discovery_section = match discovery_output {
        Ok(output) => output.trim_end().to_string(),
        Err(error) => format!("error={error}"),
    };
    let operator_section = match operator_output {
        Ok(output) => output.trim_end().to_string(),
        Err(error) => format!("error={error}"),
    };

    let mut report = format!(
        "== discover_copy_leader ==\n{}\n== operator_demo ==\n{}",
        discovery_section, operator_section
    );
    if let Some(error) = discovery_error {
        report.push_str(&format!(
            "\nflow_failure_stage=discover_copy_leader\nflow_failure_reason={error}"
        ));
    } else if let Some(error) = operator_error {
        report.push_str(&format!(
            "\nflow_failure_stage=operator_demo\nflow_failure_reason={error}"
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
    use super::{build_discover_args, parse_args, run_operator_flow, sync_selected_leader_env};
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
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
            "--skip-activity".into(),
            "--skip-discovery".into(),
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
        assert_eq!(options.proxy.as_deref(), Some("http://127.0.0.1:7897"));
        assert!(options.skip_activity);
        assert!(options.skip_discovery);
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
        assert!(!args.contains(&"--proxy".to_string()));
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

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discovery-dir".into(),
            root.join(".omx/discovery").display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should succeed");
        assert!(error.is_none());
        let report = fs::read_to_string(&report_path).expect("report exists");

        assert!(report.contains("== discover_copy_leader =="));
        assert!(report.contains("selected_wallet=0xleader"));
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

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--discover-bin".into(),
            discover.display().to_string(),
            "--operator-bin".into(),
            operator.display().to_string(),
        ])
        .expect("parse");

        let (report_path, error) = run_operator_flow(&options).expect("flow should return report");
        assert!(error.is_some());
        let report = fs::read_to_string(&report_path).expect("report exists");
        assert!(report.contains("flow_failure_stage=discover_copy_leader"));
        assert!(report.contains("discover_copy_leader"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
