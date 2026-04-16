use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

const LEADERBOARD_BASE_URL: &str = "https://data-api.polymarket.com/v1/leaderboard";
const ACTIVITY_BASE_URL: &str = "https://data-api.polymarket.com/activity";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    leaderboard_base_url: String,
    activity_base_url: String,
    category: String,
    time_period: String,
    order_by: String,
    limit: usize,
    offset: usize,
    index: usize,
    activity_type: String,
    discovery_dir: String,
    curl_bin: String,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    skip_activity: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            leaderboard_base_url: env::var("POLYMARKET_LEADERBOARD_BASE_URL")
                .unwrap_or_else(|_| LEADERBOARD_BASE_URL.to_string()),
            activity_base_url: env::var("POLYMARKET_ACTIVITY_BASE_URL")
                .unwrap_or_else(|_| ACTIVITY_BASE_URL.to_string()),
            category: "OVERALL".to_string(),
            time_period: "DAY".to_string(),
            order_by: "PNL".to_string(),
            limit: 20,
            offset: 0,
            index: 0,
            activity_type: "TRADE".to_string(),
            discovery_dir: "../.omx/discovery".to_string(),
            curl_bin: "curl".to_string(),
            connect_timeout_ms: 1_500,
            max_time_ms: 8_000,
            skip_activity: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveryArtifacts {
    selected_wallet: String,
    leaderboard_path: PathBuf,
    activity_path: Option<PathBuf>,
    selected_leader_env_path: PathBuf,
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

    match execute(&options) {
        Ok(artifacts) => {
            println!("selected_wallet={}", artifacts.selected_wallet);
            println!("leaderboard_path={}", artifacts.leaderboard_path.display());
            if let Some(activity_path) = artifacts.activity_path {
                println!("activity_path={}", activity_path.display());
            }
            println!(
                "selected_leader_env_path={}",
                artifacts.selected_leader_env_path.display()
            );
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
        "usage: discover_copy_leader [--leaderboard-base-url <url>] [--activity-base-url <url>] [--category <value>] [--time-period <value>] [--order-by <value>] [--limit <n>] [--offset <n>] [--index <n>] [--activity-type <value>] [--discovery-dir <path>] [--curl-bin <path>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--skip-activity]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--leaderboard-base-url" => options.leaderboard_base_url = next_value(&mut iter, arg)?,
            "--activity-base-url" => options.activity_base_url = next_value(&mut iter, arg)?,
            "--category" => options.category = next_value(&mut iter, arg)?,
            "--time-period" => options.time_period = next_value(&mut iter, arg)?,
            "--order-by" => options.order_by = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--offset" => options.offset = parse_usize(&next_value(&mut iter, arg)?, "offset")?,
            "--index" => options.index = parse_usize(&next_value(&mut iter, arg)?, "index")?,
            "--activity-type" => options.activity_type = next_value(&mut iter, arg)?,
            "--discovery-dir" => options.discovery_dir = next_value(&mut iter, arg)?,
            "--curl-bin" => options.curl_bin = next_value(&mut iter, arg)?,
            "--connect-timeout-ms" => {
                options.connect_timeout_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "connect-timeout-ms")?
            }
            "--max-time-ms" => {
                options.max_time_ms = parse_u64(&next_value(&mut iter, arg)?, "max-time-ms")?
            }
            "--skip-activity" => options.skip_activity = true,
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

fn execute(options: &Options) -> Result<DiscoveryArtifacts, String> {
    let discovery_dir = PathBuf::from(&options.discovery_dir);
    fs::create_dir_all(&discovery_dir)
        .map_err(|error| format!("failed to create {}: {error}", discovery_dir.display()))?;

    let leaderboard_url = build_leaderboard_url(options);
    let leaderboard_path = discovery_dir.join(format!(
        "leaderboard-{}-{}-{}.json",
        sanitize_for_filename(&options.category),
        sanitize_for_filename(&options.time_period),
        sanitize_for_filename(&options.order_by)
    ));
    let leaderboard_output = run_request(
        &options.curl_bin,
        &build_curl_args(
            &leaderboard_url,
            options.connect_timeout_ms,
            options.max_time_ms,
        ),
    )?;
    write_output_file(&leaderboard_path, &leaderboard_output.stdout).map_err(|error| {
        format!(
            "failed to write leaderboard artifact {}: {error}",
            leaderboard_path.display()
        )
    })?;

    let leaderboard_body = String::from_utf8(leaderboard_output.stdout)
        .map_err(|error| format!("leaderboard response was not utf-8: {error}"))?;
    let selected_wallet =
        extract_wallet_from_json(&leaderboard_body, options.index).ok_or_else(|| {
            format!(
                "failed to extract wallet at index {} from {}",
                options.index,
                leaderboard_path.display()
            )
        })?;

    let activity_path = if options.skip_activity {
        None
    } else {
        let activity_url = build_activity_url(
            &options.activity_base_url,
            &selected_wallet,
            &options.activity_type,
            options.limit,
        );
        let path = discovery_dir.join(format!(
            "activity-{}-{}.json",
            sanitize_for_filename(&selected_wallet),
            sanitize_for_filename(&options.activity_type.to_lowercase())
        ));
        let activity_output = run_request(
            &options.curl_bin,
            &build_curl_args(
                &activity_url,
                options.connect_timeout_ms,
                options.max_time_ms,
            ),
        )?;
        write_output_file(&path, &activity_output.stdout).map_err(|error| {
            format!(
                "failed to write activity artifact {}: {error}",
                path.display()
            )
        })?;
        Some(path)
    };

    let selected_leader_env_path = discovery_dir.join("selected-leader.env");
    write_output_file(
        &selected_leader_env_path,
        render_selected_leader_env(&selected_wallet, &leaderboard_path, options.index).as_bytes(),
    )
    .map_err(|error| {
        format!(
            "failed to write {}: {error}",
            selected_leader_env_path.display()
        )
    })?;

    Ok(DiscoveryArtifacts {
        selected_wallet,
        leaderboard_path,
        activity_path,
        selected_leader_env_path,
    })
}

fn build_leaderboard_url(options: &Options) -> String {
    format!(
        "{}?category={}&timePeriod={}&orderBy={}&limit={}&offset={}",
        options.leaderboard_base_url.trim_end_matches('/'),
        encode_component(&options.category),
        encode_component(&options.time_period),
        encode_component(&options.order_by),
        options.limit,
        options.offset
    )
}

fn build_activity_url(base_url: &str, wallet: &str, activity_type: &str, limit: usize) -> String {
    format!(
        "{}?user={}&limit={}&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type={}",
        base_url.trim_end_matches('/'),
        encode_component(wallet),
        limit,
        encode_component(activity_type)
    )
}

fn build_curl_args(url: &str, connect_timeout_ms: u64, max_time_ms: u64) -> Vec<String> {
    vec![
        "--silent".to_string(),
        "--show-error".to_string(),
        "--fail-with-body".to_string(),
        "--connect-timeout".to_string(),
        format!("{}", seconds_from_ms(connect_timeout_ms)),
        "--max-time".to_string(),
        format!("{}", seconds_from_ms(max_time_ms)),
        "-A".to_string(),
        "Mozilla/5.0".to_string(),
        "-H".to_string(),
        "Accept: application/json".to_string(),
        url.to_string(),
    ]
}

fn seconds_from_ms(value: u64) -> String {
    format!("{:.3}", value as f64 / 1_000.0)
}

fn run_request(curl_bin: &str, args: &[String]) -> Result<Output, String> {
    let output = Command::new(curl_bin)
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute {curl_bin}: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "{} exited with {}: {}{}",
            curl_bin,
            output.status.code().unwrap_or(1),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", stdout.trim())
            }
        ))
    }
}

fn extract_wallet_from_json(content: &str, index: usize) -> Option<String> {
    let fields = ["proxyWallet", "wallet", "address", "user"];
    let mut wallets = Vec::new();

    for field in fields {
        let needle = format!("\"{field}\"");
        let mut remaining = content;
        while let Some(start) = remaining.find(&needle) {
            remaining = &remaining[start + needle.len()..];
            let Some(colon) = remaining.find(':') else {
                break;
            };
            remaining = &remaining[colon + 1..];
            let trimmed = remaining.trim_start();
            if !trimmed.starts_with('"') {
                remaining = trimmed;
                continue;
            }
            let trimmed = &trimmed[1..];
            let Some(end) = trimmed.find('"') else {
                break;
            };
            let candidate = &trimmed[..end];
            if looks_like_wallet(candidate) && !wallets.iter().any(|seen| seen == candidate) {
                wallets.push(candidate.to_string());
            }
            remaining = &trimmed[end + 1..];
        }
    }

    wallets.get(index).cloned()
}

fn render_selected_leader_env(wallet: &str, leaderboard_path: &Path, index: usize) -> String {
    [
        format!("COPYTRADER_DISCOVERY_WALLET={wallet}"),
        format!("COPYTRADER_LEADER_WALLET={wallet}"),
        format!(
            "COPYTRADER_SELECTED_FROM=leaderboard:{}#{}",
            leaderboard_path.display(),
            index
        ),
    ]
    .join("\n")
        + "\n"
}

fn looks_like_wallet(value: &str) -> bool {
    value.starts_with("0x") && value.len() >= 6
}

fn write_output_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        LEADERBOARD_BASE_URL, Options, build_activity_url, build_leaderboard_url, execute,
        extract_wallet_from_json, parse_args, render_selected_leader_env, seconds_from_ms,
        write_output_file,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("discover-copy-leader-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_discovery_flags() {
        let options = parse_args(&[
            "--leaderboard-base-url".into(),
            "https://example.com/leaderboard".into(),
            "--activity-base-url".into(),
            "https://example.com/activity".into(),
            "--category".into(),
            "OVERALL".into(),
            "--time-period".into(),
            "WEEK".into(),
            "--order-by".into(),
            "VOL".into(),
            "--index".into(),
            "2".into(),
            "--activity-type".into(),
            "TRADE".into(),
            "--discovery-dir".into(),
            "/tmp/discovery".into(),
            "--skip-activity".into(),
        ])
        .expect("parse");

        assert_eq!(
            options.leaderboard_base_url,
            "https://example.com/leaderboard"
        );
        assert_eq!(options.activity_base_url, "https://example.com/activity");
        assert_eq!(options.category, "OVERALL");
        assert_eq!(options.time_period, "WEEK");
        assert_eq!(options.order_by, "VOL");
        assert_eq!(options.index, 2);
        assert_eq!(options.activity_type, "TRADE");
        assert_eq!(options.discovery_dir, "/tmp/discovery");
        assert!(options.skip_activity);
    }

    #[test]
    fn build_urls_follow_expected_shape() {
        let options = Options::default();
        let leaderboard_url = build_leaderboard_url(&options);
        let activity_url =
            build_activity_url("https://example.com/activity", "0xleader", "TRADE", 5);

        assert!(leaderboard_url.starts_with(LEADERBOARD_BASE_URL));
        assert!(leaderboard_url.contains("category=OVERALL"));
        assert!(leaderboard_url.contains("timePeriod=DAY"));
        assert!(leaderboard_url.contains("orderBy=PNL"));
        assert!(activity_url.starts_with("https://example.com/activity"));
        assert!(activity_url.contains("user=0xleader"));
        assert!(activity_url.contains("type=TRADE"));
        assert!(activity_url.contains("limit=5"));
    }

    #[test]
    fn execute_persists_discovery_artifacts_and_selected_env() {
        let root = unique_temp_dir("execute");
        fs::create_dir_all(&root).expect("temp dir created");
        let curl_stub = root.join("curl-stub.sh");
        fs::write(
            &curl_stub,
            concat!(
                "#!/usr/bin/env bash\n",
                "url=\"${@: -1}\"\n",
                "if [[ \"$url\" == *\"leaderboard\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xleader1\"},{\"proxyWallet\":\"0xleader2\"}]'\n",
                "else\n",
                "  printf '[{\"proxyWallet\":\"0xleader2\",\"side\":\"BUY\"}]'\n",
                "fi\n"
            ),
        )
        .expect("stub written");
        let mut perms = fs::metadata(&curl_stub).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&curl_stub, perms).expect("perms");

        let options = parse_args(&[
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--discovery-dir".into(),
            root.join("discovery").display().to_string(),
            "--index".into(),
            "1".into(),
        ])
        .expect("parse");

        let artifacts = execute(&options).expect("execute should succeed");

        assert_eq!(artifacts.selected_wallet, "0xleader2");
        assert!(artifacts.leaderboard_path.exists());
        assert!(
            artifacts
                .activity_path
                .as_ref()
                .expect("activity path")
                .exists()
        );
        assert!(artifacts.selected_leader_env_path.exists());
        let env = fs::read_to_string(&artifacts.selected_leader_env_path).expect("env file");
        assert!(env.contains("COPYTRADER_DISCOVERY_WALLET=0xleader2"));
        assert!(env.contains("COPYTRADER_SELECTED_FROM=leaderboard:"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn extract_wallet_from_json_finds_wallet_like_fields() {
        let json = r#"[{"proxyWallet":"0xleader1"},{"user":"0xleader2"}]"#;
        assert_eq!(
            extract_wallet_from_json(json, 0).as_deref(),
            Some("0xleader1")
        );
        assert_eq!(
            extract_wallet_from_json(json, 1).as_deref(),
            Some("0xleader2")
        );
    }

    #[test]
    fn render_selected_leader_env_includes_source_path() {
        let rendered = render_selected_leader_env("0xleader1", Path::new("/tmp/out.json"), 3);
        assert!(rendered.contains("COPYTRADER_DISCOVERY_WALLET=0xleader1"));
        assert!(rendered.contains("COPYTRADER_SELECTED_FROM=leaderboard:/tmp/out.json#3"));
    }

    #[test]
    fn write_output_file_creates_parent_directories() {
        let root = unique_temp_dir("output");
        let path = root.join("nested").join("artifact.json");
        write_output_file(&path, br#"{"ok":true}"#).expect("write should succeed");
        assert_eq!(
            fs::read_to_string(&path).expect("artifact exists"),
            "{\"ok\":true}"
        );
        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn seconds_from_ms_formats_fractional_seconds() {
        assert_eq!(seconds_from_ms(1500), "1.500");
        assert_eq!(seconds_from_ms(8000), "8.000");
    }
}
