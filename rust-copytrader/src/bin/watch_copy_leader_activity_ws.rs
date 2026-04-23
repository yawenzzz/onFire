use futures::{SinkExt, StreamExt};
use rust_copytrader::config::is_valid_evm_wallet;
use rust_copytrader::wallet_filter::parse_activity_records;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::time::Duration;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const ACTIVITY_BASE_URL: &str = "https://data-api.polymarket.com/activity";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    user: Option<String>,
    ws_url: Option<String>,
    base_url: String,
    limit: usize,
    poll_count: usize,
    activity_type: String,
    curl_bin: String,
    proxy: Option<String>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    retry_count: usize,
    retry_delay_ms: u64,
    event_wait_ms: u64,
    enrich_timeout_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            user: None,
            ws_url: env::var("POLYGON_WSS_URL").ok(),
            base_url: env::var("POLYMARKET_ACTIVITY_BASE_URL")
                .unwrap_or_else(|_| ACTIVITY_BASE_URL.to_string()),
            limit: 20,
            poll_count: 1,
            activity_type: "TRADE".to_string(),
            curl_bin: "curl".to_string(),
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            connect_timeout_ms: 8_000,
            max_time_ms: 20_000,
            retry_count: 1,
            retry_delay_ms: 500,
            event_wait_ms: 30_000,
            enrich_timeout_ms: 15_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MatchedActivity {
    tx: String,
    timestamp: String,
    side: Option<String>,
    slug: Option<String>,
    raw_body: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
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

    match watch_activity_ws(&options).await {
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
        "usage: watch_copy_leader_activity_ws [--root <path>] [--user <wallet>] [--ws-url <url>] [--base-url <url>] [--limit <n>] [--poll-count <n>] [--activity-type <value>] [--curl-bin <path>] [--proxy <url>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--retry-count <n>] [--retry-delay-ms <n>] [--event-wait-ms <n>] [--enrich-timeout-ms <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--user" => options.user = Some(next_value(&mut iter, arg)?),
            "--ws-url" => options.ws_url = Some(next_value(&mut iter, arg)?),
            "--base-url" => options.base_url = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--poll-count" => {
                options.poll_count = parse_usize(&next_value(&mut iter, arg)?, "poll-count")?
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
            "--event-wait-ms" => {
                options.event_wait_ms = parse_u64(&next_value(&mut iter, arg)?, "event-wait-ms")?
            }
            "--enrich-timeout-ms" => {
                options.enrich_timeout_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "enrich-timeout-ms")?
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

async fn watch_activity_ws(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let user = options
        .user
        .clone()
        .or_else(|| selected_leader_from_root(&root))
        .ok_or_else(|| "missing --user and no .omx/discovery/selected-leader.env found".to_string())?;
    if !is_valid_evm_wallet(&user) {
        return Err(format!("invalid watch user wallet: {user}"));
    }
    let ws_url = options
        .ws_url
        .clone()
        .or_else(|| repo_env_value(&root, "POLYGON_WSS_URL"))
        .ok_or_else(|| "missing --ws-url or POLYGON_WSS_URL".to_string())?;

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

    let (mut ws, _) = connect_async(&ws_url)
        .await
        .map_err(|error| format!("failed to connect websocket {ws_url}: {error}"))?;
    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newPendingTransactions"],
        })
        .to_string()
        .into(),
    ))
    .await
    .map_err(|error| format!("failed to subscribe websocket: {error}"))?;

    let subscription_id = wait_for_subscription_id(&mut ws, options.event_wait_ms).await?;
    let wallet_lower = user.to_ascii_lowercase();
    let mut request_id = 2_u64;
    let mut request_map = BTreeMap::<u64, String>::new();
    let mut matched = Vec::new();

    while matched.len() < options.poll_count.max(1) {
        let message = timeout(Duration::from_millis(options.event_wait_ms), ws.next())
            .await
            .map_err(|_| format!("timed out waiting for wallet activity on {ws_url}"))?
            .ok_or_else(|| "websocket closed before a matching transaction arrived".to_string())?
            .map_err(|error| format!("websocket stream error: {error}"))?;

        match message {
            Message::Text(text) => {
                let payload: Value = serde_json::from_str(&text)
                    .map_err(|error| format!("invalid websocket json: {error}"))?;
                if let Some(hash) = parse_subscription_tx_hash(&payload, &subscription_id) {
                    ws.send(Message::Text(
                        json!({
                            "jsonrpc": "2.0",
                            "id": request_id,
                            "method": "eth_getTransactionByHash",
                            "params": [hash],
                        })
                        .to_string()
                        .into(),
                    ))
                    .await
                    .map_err(|error| format!("failed to request tx details: {error}"))?;
                    request_map.insert(request_id, hash);
                    request_id += 1;
                    continue;
                }

                if let Some((response_id, tx_hash, from_address)) =
                    parse_transaction_response(&payload, &request_map)
                {
                    request_map.remove(&response_id);
                    if from_address.eq_ignore_ascii_case(&wallet_lower)
                        && !seen.contains(&tx_hash)
                    {
                        let activity = wait_for_activity_tx(options, &user, &tx_hash)?;
                        seen.insert(tx_hash.clone());
                        write_seen_txs(&seen_path, &seen).map_err(|error| {
                            format!("failed to write {}: {error}", seen_path.display())
                        })?;
                        fs::write(&latest_path, &activity.raw_body).map_err(|error| {
                            format!("failed to write {}: {error}", latest_path.display())
                        })?;
                        append_line(
                            &log_path,
                            &format!(
                                "{{\"tx\":\"{}\",\"timestamp\":\"{}\",\"side\":{},\"slug\":{}}}",
                                escape_json(&activity.tx),
                                escape_json(&activity.timestamp),
                                opt_json(activity.side.as_deref()),
                                opt_json(activity.slug.as_deref())
                            ),
                        )
                        .map_err(|error| format!("failed to append {}: {error}", log_path.display()))?;
                        matched.push(activity);
                    }
                    continue;
                }
            }
            Message::Ping(payload) => {
                ws.send(Message::Pong(payload))
                    .await
                    .map_err(|error| format!("failed to pong websocket: {error}"))?;
            }
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            Message::Close(_) => {
                return Err("websocket closed before a matching transaction arrived".to_string())
            }
        }
    }

    let mut output_lines = vec![
        format!("watch_user={user}"),
        format!("watch_state_dir={}", state_root.display()),
        format!("watch_seen_path={}", seen_path.display()),
        format!("watch_latest_path={}", latest_path.display()),
        format!("watch_log_path={}", log_path.display()),
        format!("watch_ws_url={ws_url}"),
        format!("watch_ws_subscription_id={subscription_id}"),
    ];

    for (poll_index, activity) in matched.iter().enumerate() {
        output_lines.push(format!("poll_index={poll_index}"));
        output_lines.push("poll_transport_mode=ws".to_string());
        output_lines.push("poll_new_events=1".to_string());
        output_lines.push(format!("latest_new_tx={}", activity.tx));
        output_lines.push(format!("latest_new_timestamp={}", activity.timestamp));
        if let Some(side) = &activity.side {
            output_lines.push(format!("latest_new_side={side}"));
        }
        if let Some(slug) = &activity.slug {
            output_lines.push(format!("latest_new_slug={slug}"));
        }
    }

    Ok(output_lines)
}

async fn wait_for_subscription_id(
    ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    event_wait_ms: u64,
) -> Result<String, String> {
    loop {
        let message = timeout(Duration::from_millis(event_wait_ms), ws.next())
            .await
            .map_err(|_| "timed out waiting for websocket subscription ack".to_string())?
            .ok_or_else(|| "websocket closed before subscription ack".to_string())?
            .map_err(|error| format!("websocket stream error: {error}"))?;
        if let Message::Text(text) = message {
            let payload: Value = serde_json::from_str(&text)
                .map_err(|error| format!("invalid websocket json: {error}"))?;
            if payload.get("id").and_then(Value::as_u64) == Some(1) {
                return payload
                    .get("result")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .ok_or_else(|| "missing subscription id in websocket ack".to_string());
            }
        }
    }
}

fn parse_subscription_tx_hash(payload: &Value, subscription_id: &str) -> Option<String> {
    if payload.get("method").and_then(Value::as_str) != Some("eth_subscription") {
        return None;
    }
    let params = payload.get("params")?;
    if params.get("subscription").and_then(Value::as_str) != Some(subscription_id) {
        return None;
    }
    params.get("result").and_then(Value::as_str).map(ToString::to_string)
}

fn parse_transaction_response(
    payload: &Value,
    request_map: &BTreeMap<u64, String>,
) -> Option<(u64, String, String)> {
    let response_id = payload.get("id")?.as_u64()?;
    let tx_hash = request_map.get(&response_id)?.clone();
    let result = payload.get("result")?;
    let from = result.get("from")?.as_str()?.to_ascii_lowercase();
    Some((response_id, tx_hash, from))
}

fn wait_for_activity_tx(
    options: &Options,
    user: &str,
    tx_hash: &str,
) -> Result<MatchedActivity, String> {
    let deadline = Instant::now() + Duration::from_millis(options.enrich_timeout_ms);
    while Instant::now() <= deadline {
        let url = build_activity_url(options, user);
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
        let body = String::from_utf8(output.stdout)
            .map_err(|error| format!("activity response was not utf-8: {error}"))?;
        if let Some(activity) = matched_activity_from_body(&body, tx_hash) {
            return Ok(activity);
        }
        std::thread::sleep(Duration::from_millis(options.retry_delay_ms));
    }

    Err(format!(
        "timed out waiting for tx {tx_hash} to appear in activity API"
    ))
}

fn matched_activity_from_body(body: &str, tx_hash: &str) -> Option<MatchedActivity> {
    let activity = parse_activity_records(body)
        .into_iter()
        .find(|record| record.transaction_hash.as_deref() == Some(tx_hash))?;
    Some(MatchedActivity {
        tx: tx_hash.to_string(),
        timestamp: activity.timestamp.to_string(),
        side: activity.side,
        slug: activity.slug,
        raw_body: body.to_string(),
    })
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
    let mut command = Command::new(curl_bin);
    command.args(args);
    if args.iter().any(|arg| arg == "--proxy") {
        for key in [
            "ALL_PROXY",
            "all_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
        ] {
            command.env(key, "");
        }
    }
    let output = command
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
    let mut attempts = 0usize;
    loop {
        match run_request(curl_bin, args) {
            Ok(output) => return Ok(output),
            Err(error) => {
                if attempts >= retry_count {
                    return Err(error);
                }
                attempts += 1;
                std::thread::sleep(Duration::from_millis(retry_delay_ms));
            }
        }
    }
}

fn selected_leader_from_root(root: &Path) -> Option<String> {
    let path = root.join(".omx/discovery/selected-leader.env");
    let content = fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        match key.trim() {
            "COPYTRADER_DISCOVERY_WALLET" | "COPYTRADER_LEADER_WALLET" => {
                let value = value.trim();
                (!value.is_empty() && is_valid_evm_wallet(value)).then(|| value.to_string())
            }
            _ => None,
        }
    })
}

fn repo_env_value(root: &Path, key: &str) -> Option<String> {
    let mut env_map = BTreeMap::new();
    for path in [root.join(".env.local"), root.join(".env")] {
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((candidate_key, candidate_value)) = line.split_once('=') {
                env_map.insert(
                    candidate_key.trim().to_string(),
                    candidate_value.trim().to_string(),
                );
            }
        }
    }
    env_map.get(key).cloned()
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
    use super::{Options, matched_activity_from_body, parse_args, repo_env_value, watch_activity_ws};
    use futures::{SinkExt, StreamExt};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    const WALLET: &str = "0x11084005d88A0840b5F38F8731CCa9152BbD99F7";

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("watch-copy-leader-activity-ws-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_ws_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--ws-url".into(),
            "ws://localhost:8546".into(),
            "--poll-count".into(),
            "2".into(),
            "--activity-type".into(),
            "TRADE,MERGE,SPLIT".into(),
        ])
        .expect("options parsed");

        assert_eq!(options.root, "..");
        assert_eq!(options.ws_url.as_deref(), Some("ws://localhost:8546"));
        assert_eq!(options.poll_count, 2);
        assert_eq!(options.activity_type, "TRADE,MERGE,SPLIT");
    }

    #[test]
    fn matched_activity_from_body_finds_tx() {
        let matched = matched_activity_from_body(
            r#"[{"proxyWallet":"0xabc","timestamp":123,"type":"TRADE","usdcSize":1.0,"transactionHash":"0xtx","side":"BUY","slug":"slug-1"}]"#,
            "0xtx",
        )
        .expect("matched");

        assert_eq!(matched.tx, "0xtx");
        assert_eq!(matched.timestamp, "123");
        assert_eq!(matched.side.as_deref(), Some("BUY"));
        assert_eq!(matched.slug.as_deref(), Some("slug-1"));
    }

    #[test]
    fn repo_env_value_prefers_root_env_files() {
        let root = unique_temp_dir("repo-env");
        fs::create_dir_all(&root).expect("dir created");
        fs::write(root.join(".env.local"), "POLYGON_WSS_URL=wss://local.example\n").expect("env local");
        fs::write(root.join(".env"), "POLYGON_WSS_URL=wss://root.example\n").expect("env");

        let value = repo_env_value(&root, "POLYGON_WSS_URL").expect("value");
        assert_eq!(value, "wss://root.example");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_activity_ws_detects_wallet_tx_and_writes_latest() {
        let root = unique_temp_dir("ws");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        let curl = root.join("curl");
        write_executable(
            &curl,
            "#!/bin/sh\nprintf '[{\"proxyWallet\":\"0x11084005d88A0840b5F38F8731CCa9152BbD99F7\",\"timestamp\":123,\"type\":\"TRADE\",\"usdcSize\":1.0,\"transactionHash\":\"0xtarget\",\"side\":\"BUY\",\"slug\":\"market-a\"}]'\n",
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ws");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("accept ws");

            let subscribe = ws.next().await.expect("subscribe").expect("message");
            match subscribe {
                Message::Text(_) => {}
                other => panic!("unexpected subscribe message: {other:?}"),
            }
            ws.send(Message::Text(
                r#"{"jsonrpc":"2.0","id":1,"result":"0xsub"}"#.into(),
            ))
            .await
            .expect("send ack");
            ws.send(Message::Text(
                r#"{"jsonrpc":"2.0","method":"eth_subscription","params":{"subscription":"0xsub","result":"0xtarget"}}"#.into(),
            ))
            .await
            .expect("send notif");

            let tx_lookup = ws.next().await.expect("lookup").expect("lookup msg");
            let tx_lookup_text = match tx_lookup {
                Message::Text(text) => text,
                other => panic!("unexpected lookup message: {other:?}"),
            };
            assert!(tx_lookup_text.contains("eth_getTransactionByHash"));
            ws.send(Message::Text(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"hash\":\"0xtarget\",\"from\":\"{}\"}}}}",
                    WALLET
                )
                .into(),
            ))
            .await
            .expect("send tx response");
        });

        let lines = watch_activity_ws(&Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            ws_url: Some(format!("ws://{addr}")),
            curl_bin: curl.display().to_string(),
            poll_count: 1,
            event_wait_ms: 2_000,
            enrich_timeout_ms: 2_000,
            ..Options::default()
        })
        .await
        .expect("watch should succeed");

        assert!(lines.iter().any(|line| line == "poll_transport_mode=ws"));
        assert!(lines.iter().any(|line| line == "latest_new_tx=0xtarget"));
        assert!(lines.iter().any(|line| line == "latest_new_side=BUY"));
        let latest = fs::read_to_string(root.join(format!(".omx/live-activity/{WALLET}/latest-activity.json")))
            .expect("latest exists");
        assert!(latest.contains("0xtarget"));
        let seen = fs::read_to_string(root.join(format!(".omx/live-activity/{WALLET}/seen-tx.txt")))
            .expect("seen exists");
        assert!(seen.contains("0xtarget"));
        server.await.expect("server task");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_activity_ws_reads_ws_url_from_root_env() {
        let root = unique_temp_dir("ws-root-env");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        fs::write(root.join(".env"), "POLYGON_WSS_URL=wss://example.invalid/ws\n").expect("env");

        let error = watch_activity_ws(&Options {
            root: root.display().to_string(),
            user: Some(WALLET.into()),
            ws_url: None,
            event_wait_ms: 10,
            ..Options::default()
        })
        .await
        .expect_err("should fail to connect example.invalid");
        assert!(error.contains("example.invalid"));
    }
}
