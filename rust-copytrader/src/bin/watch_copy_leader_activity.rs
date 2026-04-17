use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::Duration;

const ACTIVITY_BASE_URL: &str = "https://data-api.polymarket.com/activity";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    user: Option<String>,
    base_url: String,
    limit: usize,
    poll_count: usize,
    poll_interval_ms: u64,
    activity_type: String,
    curl_bin: String,
    proxy: Option<String>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    retry_count: usize,
    retry_delay_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            user: None,
            base_url: env::var("POLYMARKET_ACTIVITY_BASE_URL")
                .unwrap_or_else(|_| ACTIVITY_BASE_URL.to_string()),
            limit: 20,
            poll_count: 1,
            poll_interval_ms: 5_000,
            activity_type: "TRADE".to_string(),
            curl_bin: "curl".to_string(),
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            connect_timeout_ms: 8_000,
            max_time_ms: 20_000,
            retry_count: 1,
            retry_delay_ms: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EventSummary {
    tx: String,
    timestamp: String,
    side: Option<String>,
    slug: Option<String>,
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

    match watch_activity(&options) {
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
        "usage: watch_copy_leader_activity [--root <path>] [--user <wallet>] [--base-url <url>] [--limit <n>] [--poll-count <n>] [--poll-interval-ms <n>] [--activity-type <value>] [--curl-bin <path>] [--proxy <url>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--retry-count <n>] [--retry-delay-ms <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--user" => options.user = Some(next_value(&mut iter, arg)?),
            "--base-url" => options.base_url = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--poll-count" => {
                options.poll_count = parse_usize(&next_value(&mut iter, arg)?, "poll-count")?
            }
            "--poll-interval-ms" => {
                options.poll_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "poll-interval-ms")?
            }
            "--activity-type" => options.activity_type = next_value(&mut iter, arg)?,
            "--curl-bin" => options.curl_bin = next_value(&mut iter, arg)?,
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

fn watch_activity(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let user = options
        .user
        .clone()
        .or_else(|| selected_leader_from_root(&root))
        .ok_or_else(|| {
            "missing --user and no .omx/discovery/selected-leader.env found".to_string()
        })?;
    let state_root = root
        .join(".omx")
        .join("live-activity")
        .join(sanitize_for_filename(&user));
    fs::create_dir_all(&state_root)
        .map_err(|error| format!("failed to create {}: {error}", state_root.display()))?;

    let seen_path = state_root.join("seen-tx.txt");
    let latest_path = state_root.join("latest-activity.json");
    let log_path = state_root.join("activity-events.jsonl");
    let mut seen = read_seen_txs(&seen_path)
        .map_err(|error| format!("failed to read {}: {error}", seen_path.display()))?;

    let mut output_lines = vec![
        format!("watch_user={user}"),
        format!("watch_state_dir={}", state_root.display()),
        format!("watch_seen_path={}", seen_path.display()),
        format!("watch_latest_path={}", latest_path.display()),
        format!("watch_log_path={}", log_path.display()),
    ];

    for poll_index in 0..options.poll_count.max(1) {
        let url = build_activity_url(options, &user);
        let output = run_request_with_retry(
            &options.curl_bin,
            &build_curl_args(
                &url,
                options.proxy.as_deref(),
                options.connect_timeout_ms,
                options.max_time_ms,
            ),
            options.retry_count,
            options.retry_delay_ms,
        )?;
        fs::write(&latest_path, &output.stdout)
            .map_err(|error| format!("failed to write {}: {error}", latest_path.display()))?;
        let body = String::from_utf8(output.stdout)
            .map_err(|error| format!("activity response was not utf-8: {error}"))?;
        let events = extract_event_summaries(&body);
        let mut new_events = Vec::new();
        for event in events {
            if seen.insert(event.tx.clone()) {
                new_events.push(event);
            }
        }

        for event in &new_events {
            append_line(
                &log_path,
                &format!(
                    "{{\"tx\":\"{}\",\"timestamp\":\"{}\",\"side\":{},\"slug\":{}}}",
                    escape_json(&event.tx),
                    escape_json(&event.timestamp),
                    opt_json(event.side.as_deref()),
                    opt_json(event.slug.as_deref())
                ),
            )
            .map_err(|error| format!("failed to append {}: {error}", log_path.display()))?;
        }
        write_seen_txs(&seen_path, &seen)
            .map_err(|error| format!("failed to write {}: {error}", seen_path.display()))?;

        output_lines.push(format!("poll_index={poll_index}"));
        output_lines.push(format!("poll_new_events={}", new_events.len()));
        if let Some(event) = new_events.first() {
            output_lines.push(format!("latest_new_tx={}", event.tx));
            output_lines.push(format!("latest_new_timestamp={}", event.timestamp));
            if let Some(side) = &event.side {
                output_lines.push(format!("latest_new_side={side}"));
            }
            if let Some(slug) = &event.slug {
                output_lines.push(format!("latest_new_slug={slug}"));
            }
        }

        if poll_index + 1 < options.poll_count {
            thread::sleep(Duration::from_millis(options.poll_interval_ms));
        }
    }

    Ok(output_lines)
}

fn selected_leader_from_root(root: &Path) -> Option<String> {
    let path = root.join(".omx/discovery/selected-leader.env");
    let content = fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        match key.trim() {
            "COPYTRADER_DISCOVERY_WALLET" | "COPYTRADER_LEADER_WALLET" => {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            }
            _ => None,
        }
    })
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

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut current = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    current.push_str(line);
    current.push('\n');
    fs::write(path, current)
}

fn build_activity_url(options: &Options, user: &str) -> String {
    format!(
        "{}?user={}&limit={}&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type={}",
        options.base_url.trim_end_matches('/'),
        encode_component(user),
        options.limit,
        encode_component(&options.activity_type)
    )
}

fn build_curl_args(
    url: &str,
    proxy: Option<&str>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
) -> Vec<String> {
    let mut args = vec![
        "--silent".to_string(),
        "--show-error".to_string(),
        "--fail-with-body".to_string(),
        "--connect-timeout".to_string(),
        format!("{:.3}", connect_timeout_ms as f64 / 1_000.0),
        "--max-time".to_string(),
        format!("{:.3}", max_time_ms as f64 / 1_000.0),
        "-A".to_string(),
        "Mozilla/5.0".to_string(),
        "-H".to_string(),
        "Accept: application/json".to_string(),
        url.to_string(),
    ];
    if let Some(proxy) = proxy {
        args.splice(3..3, ["--proxy".to_string(), proxy.to_string()]);
    }
    args
}

fn run_request(curl_bin: &str, args: &[String]) -> Result<Output, String> {
    let output = Command::new(curl_bin)
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute {curl_bin}: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{} exited with {}: {} {}",
            curl_bin,
            output.status.code().unwrap_or(1),
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        ))
    }
}

fn run_request_with_retry(
    curl_bin: &str,
    args: &[String],
    retry_count: usize,
    retry_delay_ms: u64,
) -> Result<Output, String> {
    let mut attempts = 0;
    loop {
        match run_request(curl_bin, args) {
            Ok(output) => return Ok(output),
            Err(error) => {
                if attempts >= retry_count || !is_retryable_transport_error(&error) {
                    return Err(error);
                }
                attempts += 1;
                thread::sleep(Duration::from_millis(retry_delay_ms));
            }
        }
    }
}

fn is_retryable_transport_error(error: &str) -> bool {
    error.contains("curl exited with 28") || error.contains("curl exited with 35")
}

fn extract_event_summaries(content: &str) -> Vec<EventSummary> {
    let mut summaries = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find('{') {
        let anchor = start;
        let Some((obj_start, obj_end)) = object_bounds(rest, anchor) else {
            break;
        };
        let object = &rest[obj_start..=obj_end];
        if let Some(tx) = extract_field_value(object, "transactionHash") {
            let timestamp = extract_field_value(object, "timestamp").unwrap_or_default();
            summaries.push(EventSummary {
                tx,
                timestamp,
                side: extract_field_value(object, "side"),
                slug: extract_field_value(object, "slug"),
            });
        }
        rest = &rest[obj_end + 1..];
    }
    summaries
}

fn object_bounds(content: &str, anchor: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let start = content[..=anchor].rfind('{')?;
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, byte) in bytes.iter().enumerate().skip(start) {
        match byte {
            b'\\' if in_string && !escaped => {
                escaped = true;
                continue;
            }
            b'"' if !escaped => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, idx));
                }
            }
            _ => {}
        }
        escaped = false;
    }
    None
}

fn extract_field_value(object: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let start = object.find(&needle)?;
    let rest = object[start + needle.len()..].trim_start();
    if let Some(rest) = rest.strip_prefix('"') {
        let mut escaped = false;
        for (idx, ch) in rest.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return Some(rest[..idx].to_string()),
                _ => {}
            }
        }
        None
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
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

fn opt_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{
        EventSummary, build_activity_url, extract_event_summaries, parse_args, read_seen_txs,
        selected_leader_from_root, watch_activity, write_seen_txs,
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
        std::env::temp_dir().join(format!("watch-copy-leader-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_poll_and_proxy_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--poll-count".into(),
            "2".into(),
            "--poll-interval-ms".into(),
            "10".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
            "--retry-count".into(),
            "2".into(),
            "--retry-delay-ms".into(),
            "15".into(),
        ])
        .expect("parse");

        assert_eq!(options.poll_count, 2);
        assert_eq!(options.poll_interval_ms, 10);
        assert_eq!(options.proxy.as_deref(), Some("http://127.0.0.1:7897"));
        assert_eq!(options.retry_count, 2);
        assert_eq!(options.retry_delay_ms, 15);
    }

    #[test]
    fn selected_leader_from_root_reads_selected_env() {
        let root = unique_temp_dir("selected-env");
        fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xleader\n",
        )
        .expect("env written");

        assert_eq!(
            selected_leader_from_root(&root).as_deref(),
            Some("0xleader")
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn extract_event_summaries_reads_recent_trades() {
        let content = r#"[{"transactionHash":"0xabc","timestamp":1,"side":"BUY","slug":"market-a"},{"transactionHash":"0xdef","timestamp":2,"side":"SELL"}]"#;
        let summaries = extract_event_summaries(content);

        assert_eq!(
            summaries,
            vec![
                EventSummary {
                    tx: "0xabc".into(),
                    timestamp: "1".into(),
                    side: Some("BUY".into()),
                    slug: Some("market-a".into()),
                },
                EventSummary {
                    tx: "0xdef".into(),
                    timestamp: "2".into(),
                    side: Some("SELL".into()),
                    slug: None,
                }
            ]
        );
    }

    #[test]
    fn watch_activity_persists_latest_and_new_event_log() {
        let root = unique_temp_dir("watch");
        fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xleader\n",
        )
        .expect("env written");
        let curl_stub = root.join("curl-stub.sh");
        write_executable(
            &curl_stub,
            "#!/usr/bin/env bash\nprintf '[{\"transactionHash\":\"0xabc\",\"timestamp\":1,\"side\":\"BUY\",\"slug\":\"market-a\"},{\"transactionHash\":\"0xdef\",\"timestamp\":2,\"side\":\"SELL\"}]'\n",
        );

        let options = parse_args(&[
            "--root".into(),
            root.display().to_string(),
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--poll-count".into(),
            "1".into(),
        ])
        .expect("parse");

        let lines = watch_activity(&options).expect("watch should succeed");

        assert!(lines.contains(&"watch_user=0xleader".to_string()));
        assert!(lines.contains(&"poll_new_events=2".to_string()));
        let latest =
            fs::read_to_string(root.join(".omx/live-activity/0xleader/latest-activity.json"))
                .expect("latest activity exists");
        assert!(latest.contains("0xabc"));
        let log =
            fs::read_to_string(root.join(".omx/live-activity/0xleader/activity-events.jsonl"))
                .expect("log exists");
        assert!(log.contains("\"tx\":\"0xabc\""));
        let seen =
            read_seen_txs(&root.join(".omx/live-activity/0xleader/seen-tx.txt")).expect("seen txs");
        assert!(seen.contains("0xabc"));
        assert!(seen.contains("0xdef"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn retryable_transport_errors_cover_timeout_and_ssl_failures() {
        assert!(super::is_retryable_transport_error(
            "curl exited with 28: curl: (28) timeout"
        ));
        assert!(super::is_retryable_transport_error(
            "curl exited with 35: curl: (35) SSL_ERROR_SYSCALL"
        ));
        assert!(!super::is_retryable_transport_error(
            "curl exited with 22: HTTP 404"
        ));
    }

    #[test]
    fn write_seen_txs_persists_sorted_lines() {
        let root = unique_temp_dir("seen");
        let path = root.join("seen.txt");
        let seen = ["0xdef".to_string(), "0xabc".to_string()]
            .into_iter()
            .collect();

        write_seen_txs(&path, &seen).expect("write should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("seen file"),
            "0xabc\n0xdef\n"
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn build_activity_url_uses_user_and_type() {
        let options = parse_args(&[
            "--user".into(),
            "0xleader".into(),
            "--activity-type".into(),
            "TRADE".into(),
            "--limit".into(),
            "5".into(),
        ])
        .expect("parse");
        let url = build_activity_url(&options, "0xleader");
        assert!(url.contains("user=0xleader"));
        assert!(url.contains("type=TRADE"));
        assert!(url.contains("limit=5"));
    }
}
