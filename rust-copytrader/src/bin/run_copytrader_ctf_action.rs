use alloy::providers::ProviderBuilder;
use alloy::signers::Signer as _;
use alloy::signers::local::PrivateKeySigner;
use polymarket_client_sdk::POLYGON;
use polymarket_client_sdk::ctf::Client as CtfClient;
use polymarket_client_sdk::ctf::types::{MergePositionsRequest, SplitPositionRequest};
use polymarket_client_sdk::types::{Address as SdkAddress, B256, U256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;

const DEFAULT_RPC_URL: &str = "https://polygon-rpc.com";
const POLYGON_USDC: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    latest_activity: Option<String>,
    selected_leader_env: Option<String>,
    override_usdc_size: Option<String>,
    allow_live_submit: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            latest_activity: None,
            selected_leader_env: None,
            override_usdc_size: None,
            allow_live_submit: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LatestActivity {
    wallet: Option<String>,
    tx: String,
    timestamp: u64,
    activity_type: String,
    condition_id: Option<String>,
    outcome: Option<String>,
    slug: Option<String>,
    usdc_size: String,
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

    match run_ctf_action(&options) {
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
        "usage: run_copytrader_ctf_action [--root <path>] [--latest-activity <path>] [--selected-leader-env <path>] [--override-usdc-size <decimal>] [--allow-live-submit]"
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
            "--override-usdc-size" => {
                let value = next_value(&mut iter, arg)?;
                parse_decimal_value(&value)?;
                options.override_usdc_size = Some(value);
            }
            "--allow-live-submit" => options.allow_live_submit = true,
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

fn run_ctf_action(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let selected_leader_env = options
        .selected_leader_env
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery/selected-leader.env"));
    let leader_wallet = read_selected_leader_wallet(&selected_leader_env)?;
    let latest_activity_path = options
        .latest_activity
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(".omx/live-activity")
                .join(sanitize_for_filename(&leader_wallet))
                .join("latest-activity.json")
        });
    let latest = read_latest_activity(&latest_activity_path)?;
    let rpc_url = env::var("POLYGON_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());
    let private_key = required_merged_env(&root, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"])?;
    let signer = private_key
        .parse::<PrivateKeySigner>()
        .map_err(|error| format!("invalid PRIVATE_KEY: {error}"))?
        .with_chain_id(Some(POLYGON));
    let signer_address = signer.address().to_string();
    let action_type = latest.activity_type.to_ascii_uppercase();
    if action_type != "MERGE" && action_type != "SPLIT" {
        return Err(format!("unsupported ctf action type: {action_type}"));
    }
    let condition_id = latest
        .condition_id
        .as_deref()
        .ok_or_else(|| "missing conditionId for ctf action".to_string())?;
    let action_usdc_size = options
        .override_usdc_size
        .clone()
        .unwrap_or_else(|| latest.usdc_size.clone());
    let action_usdc_value = parse_decimal_value(&action_usdc_size)?;
    let action_amount = U256::from(usdc_to_micros(action_usdc_value)?);

    let mut lines = vec![
        "mode=ctf-action".to_string(),
        format!("selected_leader_wallet={leader_wallet}"),
        format!("selected_leader_env_path={}", selected_leader_env.display()),
        format!("latest_activity_path={}", latest_activity_path.display()),
        format!("auth_env_source={}", auth_env_source(&root)),
        format!("auth_signer_address={signer_address}"),
        format!(
            "activity_wallet={}",
            latest.wallet.as_deref().unwrap_or("unknown")
        ),
        format!("activity_tx={}", latest.tx),
        format!("activity_timestamp={}", latest.timestamp),
        format!("activity_type={action_type}"),
        format!("activity_condition_id={condition_id}"),
        format!(
            "activity_outcome={}",
            latest.outcome.as_deref().unwrap_or("unknown")
        ),
        format!(
            "activity_slug={}",
            latest.slug.as_deref().unwrap_or("unknown")
        ),
        format!("action_usdc_size={action_usdc_size}"),
        format!("action_amount_6={action_amount}"),
        format!("ctf_action_type={action_type}"),
        "ctf_action_path=rust_sdk_ctf".to_string(),
    ];

    if !options.allow_live_submit {
        lines.push("ctf_action_status=preview_only".to_string());
        return Ok(lines);
    }

    let collateral_token = SdkAddress::from_str(POLYGON_USDC)
        .map_err(|error| format!("invalid USDC address: {error}"))?;
    let condition_id = B256::from_str(condition_id)
        .map_err(|error| format!("invalid conditionId {condition_id}: {error}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;

    let action_result = runtime.block_on(async {
        let provider = ProviderBuilder::new()
            .wallet(signer)
            .connect(&rpc_url)
            .await
            .map_err(|error| format!("failed to connect polygon rpc: {error}"))?;
        let client = CtfClient::new(provider, POLYGON)
            .map_err(|error| format!("failed to initialize ctf client: {error}"))?;

        match action_type.as_str() {
            "SPLIT" => {
                let request = SplitPositionRequest::for_binary_market(
                    collateral_token,
                    condition_id,
                    action_amount,
                );
                let response = client
                    .split_position(&request)
                    .await
                    .map_err(|error| format!("failed to split position: {error}"))?;
                Ok::<_, String>((
                    response.transaction_hash.to_string(),
                    response.block_number.to_string(),
                ))
            }
            "MERGE" => {
                let request = MergePositionsRequest::for_binary_market(
                    collateral_token,
                    condition_id,
                    action_amount,
                );
                let response = client
                    .merge_positions(&request)
                    .await
                    .map_err(|error| format!("failed to merge positions: {error}"))?;
                Ok::<_, String>((
                    response.transaction_hash.to_string(),
                    response.block_number.to_string(),
                ))
            }
            _ => unreachable!(),
        }
    })?;

    lines.push("ctf_action_status=submitted".to_string());
    lines.push(format!("ctf_action_tx_hash={}", action_result.0));
    lines.push(format!("ctf_action_block_number={}", action_result.1));

    Ok(lines)
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

fn read_latest_activity(path: &Path) -> Result<LatestActivity, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let object = first_json_object(&body).ok_or_else(|| {
        format!(
            "failed to parse latest activity JSON from {}",
            path.display()
        )
    })?;
    Ok(LatestActivity {
        wallet: extract_field_value(&object, "proxyWallet")
            .or_else(|| extract_field_value(&object, "wallet")),
        tx: extract_field_value(&object, "transactionHash")
            .ok_or_else(|| "missing transactionHash in latest activity".to_string())?,
        timestamp: extract_field_value(&object, "timestamp")
            .ok_or_else(|| "missing timestamp in latest activity".to_string())?
            .parse::<u64>()
            .map_err(|error| format!("invalid timestamp: {error}"))?,
        activity_type: extract_field_value(&object, "type").unwrap_or_else(|| "TRADE".to_string()),
        condition_id: extract_field_value(&object, "conditionId"),
        outcome: extract_field_value(&object, "outcome"),
        slug: extract_field_value(&object, "slug"),
        usdc_size: extract_field_value(&object, "usdcSize")
            .ok_or_else(|| "missing usdcSize in latest activity".to_string())?,
    })
}

fn first_json_object(content: &str) -> Option<String> {
    let start = content.find('{')?;
    let (from, to) = object_bounds(content, start)?;
    Some(content[from..=to].to_string())
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

fn parse_decimal_value(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("invalid decimal: {value}"))
}

fn usdc_to_micros(value: f64) -> Result<u64, String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("invalid action usdc size: {value}"));
    }
    Ok((value * 1_000_000.0).round() as u64)
}

fn auth_env_source(root: &Path) -> &'static str {
    if root.join(".env").exists() {
        ".env"
    } else if root.join(".env.local").exists() {
        ".env.local"
    } else {
        "environment"
    }
}

fn required_merged_env(root: &Path, keys: &[&str]) -> Result<String, String> {
    let env_map = merged_env(root)?;
    keys.iter()
        .find_map(|key| env_map.get(*key).cloned())
        .ok_or_else(|| format!("missing field {}", keys[0]))
}

fn merged_env(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut env_map = BTreeMap::new();
    let env_local = root.join(".env.local");
    if env_local.exists() {
        merge_env_file(&env_local, &mut env_map)?;
    }
    let env = root.join(".env");
    if env.exists() {
        merge_env_file(&env, &mut env_map)?;
    }
    Ok(env_map)
}

fn merge_env_file(path: &Path, env_map: &mut BTreeMap<String, String>) -> Result<(), String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env_map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{Options, parse_args, read_latest_activity, run_ctf_action};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-ctf-action-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_preview_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--latest-activity".into(),
            "latest.json".into(),
            "--override-usdc-size".into(),
            "0.5".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert_eq!(options.latest_activity.as_deref(), Some("latest.json"));
        assert_eq!(options.override_usdc_size.as_deref(), Some("0.5"));
        assert!(!options.allow_live_submit);
    }

    #[test]
    fn read_latest_activity_parses_merge_event() {
        let root = unique_temp_dir("merge");
        fs::create_dir_all(&root).expect("dir created");
        let latest = root.join("latest.json");
        fs::write(
            &latest,
            r#"[{"proxyWallet":"0xabc","timestamp":1776782652,"type":"MERGE","conditionId":"0xcond","usdcSize":3.5,"transactionHash":"0xtx","outcome":"No","slug":"market-a"}]"#,
        )
        .expect("latest written");

        let activity = read_latest_activity(&latest).expect("activity");
        assert_eq!(activity.activity_type, "MERGE");
        assert_eq!(activity.condition_id.as_deref(), Some("0xcond"));
        assert_eq!(activity.usdc_size, "3.5");
    }

    #[test]
    fn run_ctf_action_reports_preview_for_split_event() {
        let root = unique_temp_dir("preview");
        fs::create_dir_all(root.join(".omx/discovery")).expect("dir created");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xabc\nCOPYTRADER_LEADER_WALLET=0xabc\n",
        )
        .expect("selected env");
        fs::write(
            root.join(".env"),
            "PRIVATE_KEY=0x59c6995e998f97a5a0044966f094538c5f34f6c4a0499b6f6f489f5fabe59d3f\n",
        )
        .expect("env");
        let latest = root.join("latest.json");
        fs::write(
            &latest,
            r#"[{"proxyWallet":"0xabc","timestamp":1776782652,"type":"SPLIT","conditionId":"0x1111111111111111111111111111111111111111111111111111111111111111","usdcSize":0.5,"transactionHash":"0xtx","outcome":"No","slug":"market-a"}]"#,
        )
        .expect("latest written");

        let lines = run_ctf_action(&Options {
            root: root.display().to_string(),
            latest_activity: Some(latest.display().to_string()),
            selected_leader_env: Some(
                root.join(".omx/discovery/selected-leader.env")
                    .display()
                    .to_string(),
            ),
            override_usdc_size: None,
            allow_live_submit: false,
        })
        .expect("preview");
        assert!(lines.iter().any(|line| line == "ctf_action_type=SPLIT"));
        assert!(
            lines
                .iter()
                .any(|line| line == "ctf_action_status=preview_only")
        );
        assert!(lines.iter().any(|line| line == "action_usdc_size=0.5"));
    }
}
