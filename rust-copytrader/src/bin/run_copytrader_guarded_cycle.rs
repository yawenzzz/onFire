use rust_copytrader::app::{RuntimeSession, RuntimeSessionRecorder, SessionOutcome};
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::replay::fixture::{ReplayFixture, ReplayQuoteFrame, ReplayVerificationFrame};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    latest_activity: Option<String>,
    selected_leader_env: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            latest_activity: None,
            selected_leader_env: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestActivityEvent {
    tx: String,
    timestamp: u64,
    side: Option<String>,
    slug: Option<String>,
}

fn main() -> std::process::ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return std::process::ExitCode::SUCCESS;
    }

    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            return std::process::ExitCode::from(2);
        }
    };

    match run_guarded_cycle(&options) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!(
        "usage: run_copytrader_guarded_cycle [--root <path>] [--latest-activity <path>] [--selected-leader-env <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--latest-activity" => options.latest_activity = Some(next_value(&mut iter, arg)?),
            "--selected-leader-env" => {
                options.selected_leader_env = Some(next_value(&mut iter, arg)?)
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

fn run_guarded_cycle(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let selected_leader_env = options
        .selected_leader_env
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery/selected-leader.env"));
    let latest_activity_path = options
        .latest_activity
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let leader_wallet =
                read_selected_leader_wallet(&selected_leader_env).unwrap_or_default();
            root.join(".omx/live-activity")
                .join(sanitize_for_filename(&leader_wallet))
                .join("latest-activity.json")
        });

    let leader_wallet = read_selected_leader_wallet(&selected_leader_env)?;
    let latest_event = read_latest_activity_event(&latest_activity_path)?;

    let fixture = build_replay_fixture(&leader_wallet, &latest_event);
    let gate = LiveModeGate::for_mode(ActivityMode::Replay);
    let mut session =
        RuntimeSession::from_root(ActivityMode::Replay, gate, &root).map_err(format_root_error)?;
    let outcome = session.process_replay(&fixture);

    let cycle_root = root.join(".omx").join("guarded-cycle");
    let session_id = format!("guarded-cycle-{}", now_nanos()?);
    let mut recorder = RuntimeSessionRecorder::new(&cycle_root, &session_id, 8, 64);
    let persisted = recorder
        .persist(&session)
        .map_err(|error| format!("failed to persist guarded cycle artifacts: {error}"))?;
    let snapshot = session
        .snapshot()
        .ok_or_else(|| "missing runtime snapshot".to_string())?;

    let mut lines = vec![
        "mode=guarded-cycle".to_string(),
        format!("selected_leader_wallet={leader_wallet}"),
        format!("selected_leader_env_path={}", selected_leader_env.display()),
        format!("latest_activity_path={}", latest_activity_path.display()),
        format!("cycle_activity_tx={}", latest_event.tx),
        format!("cycle_activity_timestamp={}", latest_event.timestamp),
        format!(
            "cycle_activity_side={}",
            latest_event.side.as_deref().unwrap_or("unknown")
        ),
        format!(
            "cycle_activity_slug={}",
            latest_event.slug.as_deref().unwrap_or("unknown")
        ),
        format!(
            "cycle_outcome={}",
            match outcome {
                SessionOutcome::Blocked(ref reason) => format!("blocked:{reason}"),
                SessionOutcome::Processed => "processed".to_string(),
            }
        ),
        format!("runtime_mode={}", snapshot.runtime.mode),
        format!("last_submit_status={}", snapshot.runtime.last_submit_status),
        format!(
            "last_total_elapsed_ms={}",
            snapshot.runtime.last_total_elapsed_ms
        ),
        format!(
            "latest_snapshot_path={}",
            persisted.latest_snapshot_path.display()
        ),
        format!("report_path={}", persisted.report_path.display()),
        format!("summary_path={}", persisted.summary_path.display()),
    ];

    if let Some(value) = &snapshot.runtime.selected_leader_rank {
        lines.push(format!("selected_leader_rank={value}"));
    }
    if let Some(value) = &snapshot.runtime.selected_leader_pnl {
        lines.push(format!("selected_leader_pnl={value}"));
    }
    if let Some(value) = &snapshot.runtime.selected_leader_username {
        lines.push(format!("selected_leader_username={value}"));
    }
    if let Some(value) = &snapshot.runtime.selected_leader_latest_activity_side {
        lines.push(format!("selected_leader_latest_activity_side={value}"));
    }
    if let Some(value) = &snapshot.runtime.selected_leader_latest_activity_slug {
        lines.push(format!("selected_leader_latest_activity_slug={value}"));
    }

    Ok(lines)
}

fn build_replay_fixture(wallet: &str, event: &LatestActivityEvent) -> ReplayFixture {
    let mut fixture = ReplayFixture::success_buy_follow().with_leader_wallet(wallet.to_string());
    fixture.leader_id = wallet.to_string();
    fixture.correlation_id = format!("corr-{}", event.tx);
    fixture.activity.transaction_hash = event.tx.clone();
    fixture.activity.observed_at_ms = event.timestamp;
    fixture.activity.side = event.side.clone().unwrap_or_else(|| "BUY".to_string());
    fixture.previous_position.observed_at_ms = event.timestamp.saturating_sub(20);
    fixture.current_position.observed_at_ms = event.timestamp.saturating_add(20);
    fixture.positions_reconciled_at_ms = event.timestamp.saturating_add(20);
    fixture.quote = ReplayQuoteFrame::new(
        fixture.quote.asset_id.clone(),
        fixture.quote.best_bid,
        fixture.quote.best_ask,
        fixture.quote.quote_age_ms,
        event.timestamp.saturating_add(28),
    );
    fixture.submit_ack_at_ms = event.timestamp.saturating_add(60);
    fixture.verification = ReplayVerificationFrame::Verified {
        verified_at_ms: event.timestamp.saturating_add(82),
    };
    fixture
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

fn read_latest_activity_event(path: &Path) -> Result<LatestActivityEvent, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let object = first_json_object(&body).ok_or_else(|| {
        format!(
            "failed to parse latest activity JSON from {}",
            path.display()
        )
    })?;
    let tx = extract_field_value(&object, "transactionHash")
        .ok_or_else(|| "missing transactionHash in latest activity".to_string())?;
    let timestamp = extract_field_value(&object, "timestamp")
        .ok_or_else(|| "missing timestamp in latest activity".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("invalid latest activity timestamp: {error}"))?;

    Ok(LatestActivityEvent {
        tx,
        timestamp,
        side: extract_field_value(&object, "side"),
        slug: extract_field_value(&object, "slug"),
    })
}

fn first_json_object(content: &str) -> Option<String> {
    let start = content.find('{')?;
    object_bounds(content, start).map(|(start, end)| content[start..=end].to_string())
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

fn now_nanos() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time error: {error}"))
        .map(|duration| duration.as_nanos())
}

fn format_root_error(error: rust_copytrader::config::RootEnvLoadError) -> String {
    match error {
        rust_copytrader::config::RootEnvLoadError::Io { path, error } => {
            format!("io error at {}: {error}", path.display())
        }
        rust_copytrader::config::RootEnvLoadError::MissingField(field) => {
            format!("missing field {field}")
        }
        rust_copytrader::config::RootEnvLoadError::InvalidNumber { field, value } => {
            format!("invalid number for {field}: {value}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_replay_fixture, first_json_object, parse_args, read_latest_activity_event,
        read_selected_leader_wallet, sanitize_for_filename,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-guarded-cycle-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_root_and_paths() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--latest-activity".into(),
            "/tmp/latest.json".into(),
            "--selected-leader-env".into(),
            "/tmp/leader.env".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert_eq!(options.latest_activity.as_deref(), Some("/tmp/latest.json"));
        assert_eq!(
            options.selected_leader_env.as_deref(),
            Some("/tmp/leader.env")
        );
    }

    #[test]
    fn read_selected_leader_wallet_prefers_copytrader_env_key() {
        let root = unique_temp_dir("leader-env");
        fs::create_dir_all(&root).expect("temp dir created");
        let env_path = root.join("selected-leader.env");
        fs::write(&env_path, "COPYTRADER_DISCOVERY_WALLET=0xleader\n").expect("env written");

        assert_eq!(
            read_selected_leader_wallet(&env_path).expect("wallet"),
            "0xleader"
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn read_latest_activity_event_extracts_transaction_and_timestamp() {
        let root = unique_temp_dir("latest-activity");
        fs::create_dir_all(&root).expect("temp dir created");
        let latest = root.join("latest.json");
        fs::write(
            &latest,
            r#"[{"transactionHash":"0xabc","timestamp":1776303488,"side":"BUY","slug":"market-a"}]"#,
        )
        .expect("latest written");

        let event = read_latest_activity_event(&latest).expect("event");

        assert_eq!(event.tx, "0xabc");
        assert_eq!(event.timestamp, 1_776_303_488);
        assert_eq!(event.side.as_deref(), Some("BUY"));
        assert_eq!(event.slug.as_deref(), Some("market-a"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn build_replay_fixture_reanchors_fixture_to_real_activity_event() {
        let event = super::LatestActivityEvent {
            tx: "0xabc".into(),
            timestamp: 1_776_303_488,
            side: Some("SELL".into()),
            slug: Some("market-a".into()),
        };
        let fixture = build_replay_fixture("0xleader", &event);

        assert_eq!(fixture.leader_id, "0xleader");
        assert_eq!(fixture.activity.proxy_wallet, "0xleader");
        assert_eq!(fixture.activity.transaction_hash, "0xabc");
        assert_eq!(fixture.activity.side, "SELL");
        assert_eq!(fixture.submit_ack_at_ms, 1_776_303_548);
    }

    #[test]
    fn first_json_object_returns_first_event_object() {
        let object =
            first_json_object(r#"[{"transactionHash":"0xabc"},{"transactionHash":"0xdef"}]"#)
                .expect("object");
        assert!(object.contains("\"transactionHash\":\"0xabc\""));
    }

    #[test]
    fn sanitize_for_filename_replaces_non_portable_chars() {
        assert_eq!(sanitize_for_filename("0xabc:def"), "0xabc_def");
    }
}
