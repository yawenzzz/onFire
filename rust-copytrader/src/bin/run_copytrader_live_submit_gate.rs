use rust_copytrader::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use rust_copytrader::adapters::http_submit::{
    HttpSubmitRequestBuilder, HttpSubmitter, OrderBatchRequest, OrderType,
};
use rust_copytrader::adapters::signing::{
    AuthMaterial, CommandL2HeaderSigner, CommandOrderSigner, L2HeaderSigningPayload,
    StdSigningCommandRunner, StdSigningCommandRunner as HeaderRunner, UnsignedOrderPayload,
    prepare_l2_auth_headers, prepare_signed_order,
};
use rust_copytrader::config::{ActivityMode, ExecutionAdapterConfig, LiveModeGate, RootEnvLoadError};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    latest_activity: Option<String>,
    selected_leader_env: Option<String>,
    owner: Option<String>,
    order_type: OrderType,
    expiration_secs: u64,
    fee_rate_bps: u64,
    activity_source_verified: bool,
    activity_under_budget: bool,
    activity_capability_detected: bool,
    positions_under_budget: bool,
    allow_live_submit: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            latest_activity: None,
            selected_leader_env: None,
            owner: None,
            order_type: OrderType::Fak,
            expiration_secs: 300,
            fee_rate_bps: 30,
            activity_source_verified: env_flag("COPYTRADER_LIVE_ACTIVITY_VERIFIED"),
            activity_under_budget: env_flag("COPYTRADER_LIVE_ACTIVITY_UNDER_BUDGET"),
            activity_capability_detected: env_flag("COPYTRADER_LIVE_ACTIVITY_CAPABILITY_DETECTED"),
            positions_under_budget: env_flag("COPYTRADER_LIVE_POSITIONS_UNDER_BUDGET"),
            allow_live_submit: env_flag("COPYTRADER_ALLOW_LIVE_SUBMIT"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LatestActivity {
    tx: String,
    timestamp: u64,
    side: String,
    slug: Option<String>,
    asset: String,
    size: String,
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

    match run_live_submit_gate(&options) {
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
        "usage: run_copytrader_live_submit_gate [--root <path>] [--latest-activity <path>] [--selected-leader-env <path>] [--owner <value>] [--order-type <GTC|GTD|FOK|FAK>] [--expiration-secs <n>] [--fee-rate-bps <n>] [--activity-source-verified] [--activity-under-budget] [--activity-capability-detected] [--positions-under-budget] [--allow-live-submit]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--latest-activity" => options.latest_activity = Some(next_value(&mut iter, arg)?),
            "--selected-leader-env" => options.selected_leader_env = Some(next_value(&mut iter, arg)?),
            "--owner" => options.owner = Some(next_value(&mut iter, arg)?),
            "--order-type" => options.order_type = parse_order_type(&next_value(&mut iter, arg)?)?,
            "--expiration-secs" => {
                options.expiration_secs =
                    parse_u64(&next_value(&mut iter, arg)?, "expiration-secs")?
            }
            "--fee-rate-bps" => {
                options.fee_rate_bps = parse_u64(&next_value(&mut iter, arg)?, "fee-rate-bps")?
            }
            "--activity-source-verified" => options.activity_source_verified = true,
            "--activity-under-budget" => options.activity_under_budget = true,
            "--activity-capability-detected" => options.activity_capability_detected = true,
            "--positions-under-budget" => options.positions_under_budget = true,
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

fn parse_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_order_type(value: &str) -> Result<OrderType, String> {
    match value {
        "GTC" => Ok(OrderType::Gtc),
        "GTD" => Ok(OrderType::Gtd),
        "FOK" => Ok(OrderType::Fok),
        "FAK" => Ok(OrderType::Fak),
        other => Err(format!("unsupported order type: {other}")),
    }
}

fn run_live_submit_gate(options: &Options) -> Result<Vec<String>, String> {
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
            let wallet = read_selected_leader_wallet(&selected_leader_env).unwrap_or_default();
            root.join(".omx/live-activity")
                .join(sanitize_for_filename(&wallet))
                .join("latest-activity.json")
        });

    let leader_wallet = read_selected_leader_wallet(&selected_leader_env)?;
    let latest = read_latest_activity(&latest_activity_path)?;
    let execution_config =
        ExecutionAdapterConfig::from_root(&root).map_err(format_root_error)?;
    let material = AuthMaterial::from_root(&root).map_err(format_root_error)?;

    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = options.activity_source_verified;
    gate.activity_source_under_budget = options.activity_under_budget;
    gate.activity_capability_detected = options.activity_capability_detected;
    gate.positions_under_budget = options.positions_under_budget;
    gate.execution_surface_ready = execution_config.live_ready();

    let unsigned = unsigned_order_from_activity(&latest, options)?;
    let report_path = live_submit_report_path(&root)?;
    let owner = options
        .owner
        .clone()
        .unwrap_or_else(|| material.poly_address.clone());

    let mut lines = vec![
        "mode=live-submit-gate".to_string(),
        format!("selected_leader_wallet={leader_wallet}"),
        format!("selected_leader_env_path={}", selected_leader_env.display()),
        format!("latest_activity_path={}", latest_activity_path.display()),
        format!("activity_tx={}", latest.tx),
        format!("activity_timestamp={}", latest.timestamp),
        format!("activity_side={}", latest.side),
        format!("activity_slug={}", latest.slug.as_deref().unwrap_or("unknown")),
        format!("activity_asset={}", latest.asset),
        format!("unsigned_maker_amount={}", unsigned.maker_amount),
        format!("unsigned_taker_amount={}", unsigned.taker_amount),
        format!("gate_execution_surface_ready={}", gate.execution_surface_ready),
    ];

    if let Some(reason) = gate.blocked_reason() {
        lines.push(format!("live_gate_status=blocked:{reason}"));
        lines.push(format!("report_path={}", report_path.display()));
        write_report(&report_path, &lines)?;
        return Ok(lines);
    }

    lines.push("live_gate_status=unlocked".to_string());

    let wiring = execution_config
        .live_execution_wiring()
        .ok_or_else(|| "missing live execution wiring".to_string())?;
    let l2_helper = execution_config
        .live_l2_header_helper()
        .ok_or_else(|| "missing live l2 helper".to_string())?;

    let mut order_signer = CommandOrderSigner::new(
        wiring.signing.program.clone(),
        wiring.signing.args.clone(),
        StdSigningCommandRunner,
    );
    let mut l2_signer = CommandL2HeaderSigner::new(
        l2_helper.program.clone(),
        l2_helper.args.clone(),
        HeaderRunner,
    );

    let signed = prepare_signed_order(
        &material,
        unsigned.clone(),
        owner.clone(),
        options.order_type,
        false,
        &mut order_signer,
    )
    .map_err(|error| format!("failed to sign order: {error:?}"))?;
    let batch = OrderBatchRequest::single(signed);

    let auth = AuthRuntimeState::new(
        !material.api_key.is_empty() && !material.passphrase.is_empty(),
        !material.private_key.is_empty(),
        true,
        material.signature_type,
        material.funder.is_some(),
    );
    let dummy_headers = L2AuthHeaders::new(
        material.poly_address.clone(),
        material.api_key.clone(),
        material.passphrase.clone(),
        "pending",
        "0",
    );
    let builder = HttpSubmitRequestBuilder::new(&wiring.submit_base_url);
    let provisional = builder
        .build(&auth, &dummy_headers, &batch)
        .map_err(|error| format!("failed to build provisional request: {error:?}"))?;
    let headers = prepare_l2_auth_headers(
        &material,
        L2HeaderSigningPayload {
            method: "POST".into(),
            request_path: "/orders".into(),
            body: provisional.body.clone(),
        },
        &mut l2_signer,
    )
    .map_err(|error| format!("failed to sign l2 headers: {error:?}"))?;

    let submitter =
        HttpSubmitter::from_execution_config(&execution_config).map_err(|error| format!(
            "failed to build submitter from execution config: {error:?}"
        ))?;

    if !options.allow_live_submit {
        let preview = submitter
            .preview_command(&auth, &headers, &batch)
            .map_err(|error| format!("failed to build live preview command: {error:?}"))?;
        lines.push("live_submit_status=preview_only".to_string());
        lines.push(format!("preview_program={}", preview.program));
        lines.push(format!("preview_args={}", preview.args.join(" ")));
        lines.push(format!("report_path={}", report_path.display()));
        write_report(&report_path, &lines)?;
        return Ok(lines);
    }

    let mut runner = rust_copytrader::adapters::http_submit::StdCommandRunner;
    let result = submitter
        .submit(&auth, &headers, &batch, &mut runner)
        .map_err(|error| format!("live submit failed: {error:?}"))?;

    lines.push("live_submit_status=submitted".to_string());
    lines.push(format!("submit_status_code={}", result.response.status_code));
    lines.push(format!("submit_response_body={}", result.response.body));
    lines.push(format!("report_path={}", report_path.display()));
    write_report(&report_path, &lines)?;
    Ok(lines)
}

fn unsigned_order_from_activity(
    latest: &LatestActivity,
    options: &Options,
) -> Result<UnsignedOrderPayload, String> {
    let size = decimal_to_fixed_6(&latest.size)?;
    let usdc_size = decimal_to_fixed_6(&latest.usdc_size)?;
    let side = latest.side.to_uppercase();
    let (maker_amount, taker_amount) = match side.as_str() {
        "BUY" => (usdc_size.clone(), size.clone()),
        "SELL" => (size.clone(), usdc_size.clone()),
        other => return Err(format!("unsupported activity side: {other}")),
    };

    Ok(UnsignedOrderPayload {
        taker: "0x0000000000000000000000000000000000000000".into(),
        token_id: latest.asset.clone(),
        maker_amount,
        taker_amount,
        side,
        expiration: (latest.timestamp + options.expiration_secs).to_string(),
        nonce: latest.timestamp.to_string(),
        fee_rate_bps: options.fee_rate_bps.to_string(),
    })
}

fn decimal_to_fixed_6(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("empty decimal value".to_string());
    }
    let negative = trimmed.starts_with('-');
    let trimmed = trimmed.trim_start_matches('-');
    let (whole, frac) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    let whole = whole
        .chars()
        .filter(|ch| *ch != ',')
        .collect::<String>();
    if !whole.chars().all(|ch| ch.is_ascii_digit()) || !frac.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(format!("invalid decimal value: {value}"));
    }
    let mut frac = frac.to_string();
    while frac.len() < 6 {
        frac.push('0');
    }
    let frac = &frac[..6.min(frac.len())];
    let combined = format!("{}{}", whole, frac);
    let combined = combined.trim_start_matches('0');
    let normalized = if combined.is_empty() { "0" } else { combined };
    if negative {
        Ok(format!("-{normalized}"))
    } else {
        Ok(normalized.to_string())
    }
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
    let object = first_json_object(&body)
        .ok_or_else(|| format!("failed to parse latest activity JSON from {}", path.display()))?;
    Ok(LatestActivity {
        tx: extract_field_value(&object, "transactionHash")
            .ok_or_else(|| "missing transactionHash in latest activity".to_string())?,
        timestamp: extract_field_value(&object, "timestamp")
            .ok_or_else(|| "missing timestamp in latest activity".to_string())?
            .parse::<u64>()
            .map_err(|error| format!("invalid latest activity timestamp: {error}"))?,
        side: extract_field_value(&object, "side")
            .ok_or_else(|| "missing side in latest activity".to_string())?,
        slug: extract_field_value(&object, "slug"),
        asset: extract_field_value(&object, "asset")
            .ok_or_else(|| "missing asset in latest activity".to_string())?,
        size: extract_field_value(&object, "size")
            .ok_or_else(|| "missing size in latest activity".to_string())?,
        usdc_size: extract_field_value(&object, "usdcSize")
            .ok_or_else(|| "missing usdcSize in latest activity".to_string())?,
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

fn live_submit_report_path(root: &Path) -> Result<PathBuf, String> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time error: {error}"))?
        .as_nanos();
    Ok(root
        .join(".omx")
        .join("live-submit")
        .join(format!("live-submit-{run_id}.txt")))
}

fn write_report(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, lines.join("\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn format_root_error(error: RootEnvLoadError) -> String {
    match error {
        RootEnvLoadError::Io { path, error } => format!("io error at {}: {error}", path.display()),
        RootEnvLoadError::MissingField(field) => format!("missing field {field}"),
        RootEnvLoadError::InvalidNumber { field, value } => {
            format!("invalid number for {field}: {value}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LatestActivity, Options, decimal_to_fixed_6, parse_args, read_latest_activity, unsigned_order_from_activity};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-live-submit-gate-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_gate_and_submit_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--activity-source-verified".into(),
            "--activity-under-budget".into(),
            "--activity-capability-detected".into(),
            "--positions-under-budget".into(),
            "--allow-live-submit".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert!(options.activity_source_verified);
        assert!(options.activity_under_budget);
        assert!(options.activity_capability_detected);
        assert!(options.positions_under_budget);
        assert!(options.allow_live_submit);
    }

    #[test]
    fn decimal_to_fixed_6_converts_decimal_strings_to_micros() {
        assert_eq!(decimal_to_fixed_6("1.23").expect("micros"), "1230000");
        assert_eq!(decimal_to_fixed_6("0.000001").expect("micros"), "1");
        assert_eq!(decimal_to_fixed_6("12").expect("micros"), "12000000");
    }

    #[test]
    fn read_latest_activity_extracts_asset_and_sizes() {
        let root = unique_temp_dir("latest");
        fs::create_dir_all(&root).expect("temp dir created");
        let latest = root.join("latest.json");
        fs::write(
            &latest,
            r#"[{"transactionHash":"0xabc","timestamp":1776303488,"side":"BUY","slug":"market-a","asset":"12345","size":138.6735,"usdcSize":63.45}]"#,
        )
        .expect("latest written");

        let activity = read_latest_activity(&latest).expect("activity");
        assert_eq!(activity.asset, "12345");
        assert_eq!(activity.size, "138.6735");
        assert_eq!(activity.usdc_size, "63.45");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn unsigned_order_from_activity_swaps_amounts_for_sell_side() {
        let event = LatestActivity {
            tx: "0xabc".into(),
            timestamp: 1_776_303_488,
            side: "SELL".into(),
            slug: Some("market-a".into()),
            asset: "12345".into(),
            size: "10".into(),
            usdc_size: "5".into(),
        };
        let order = unsigned_order_from_activity(&event, &Options::default()).expect("order");

        assert_eq!(order.token_id, "12345");
        assert_eq!(order.side, "SELL");
        assert_eq!(order.maker_amount, "10000000");
        assert_eq!(order.taker_amount, "5000000");
    }
}
