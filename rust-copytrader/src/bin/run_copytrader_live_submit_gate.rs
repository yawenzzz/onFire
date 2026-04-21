use polymarket_client_sdk::auth::{Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid};
use polymarket_client_sdk::clob::types::{
    Amount as SdkAmount, OrderType as SdkOrderType, Side as SdkSide,
    SignableOrder as SdkSignableOrder, SignatureType as SdkSignatureType,
    SignedOrder as SdkSignedOrder,
};
use polymarket_client_sdk::clob::{Client as SdkClobClient, Config as SdkClobConfig};
use polymarket_client_sdk::types::{Address as SdkAddress, Decimal as SdkDecimal};
use polymarket_client_sdk::{POLYGON, derive_proxy_wallet, derive_safe_wallet};
use rust_copytrader::adapters::http_submit::OrderType;
use rust_copytrader::adapters::signing::{AuthMaterial, UnsignedOrderPayload};
use rust_copytrader::config::{
    ActivityMode, ExecutionAdapterConfig, LiveModeGate, RootEnvLoadError, is_valid_evm_wallet,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Builder as TokioRuntimeBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    latest_activity: Option<String>,
    selected_leader_env: Option<String>,
    owner: Option<String>,
    override_usdc_size: Option<String>,
    max_total_exposure_usdc: Option<String>,
    max_order_usdc: Option<String>,
    account_snapshot: Option<String>,
    account_snapshot_max_age_secs: u64,
    activity_max_age_secs: u64,
    order_type: OrderType,
    expiration_secs: u64,
    fee_rate_bps: u64,
    activity_source_verified: bool,
    activity_under_budget: bool,
    activity_capability_detected: bool,
    positions_under_budget: bool,
    allow_live_submit: bool,
    force_live_submit: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            latest_activity: None,
            selected_leader_env: None,
            owner: None,
            override_usdc_size: None,
            max_total_exposure_usdc: env_value("COPYTRADER_MAX_TOTAL_EXPOSURE_USDC"),
            max_order_usdc: env_value("COPYTRADER_MAX_ORDER_USDC"),
            account_snapshot: env_value("COPYTRADER_ACCOUNT_SNAPSHOT_PATH"),
            account_snapshot_max_age_secs: env_u64("COPYTRADER_ACCOUNT_SNAPSHOT_MAX_AGE_SECS", 300),
            activity_max_age_secs: env_u64("COPYTRADER_ACTIVITY_MAX_AGE_SECS", 60),
            order_type: OrderType::Fak,
            expiration_secs: 300,
            fee_rate_bps: 30,
            activity_source_verified: env_flag("COPYTRADER_LIVE_ACTIVITY_VERIFIED"),
            activity_under_budget: env_flag("COPYTRADER_LIVE_ACTIVITY_UNDER_BUDGET"),
            activity_capability_detected: env_flag("COPYTRADER_LIVE_ACTIVITY_CAPABILITY_DETECTED"),
            positions_under_budget: env_flag("COPYTRADER_LIVE_POSITIONS_UNDER_BUDGET"),
            allow_live_submit: env_flag("COPYTRADER_ALLOW_LIVE_SUBMIT"),
            force_live_submit: env_flag("COPYTRADER_FORCE_LIVE_SUBMIT"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LatestActivity {
    wallet: Option<String>,
    tx: String,
    timestamp: u64,
    price: Option<f64>,
    side: String,
    slug: Option<String>,
    asset: String,
    size: String,
    usdc_size: String,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedOrderDraft {
    unsigned: UnsignedOrderPayload,
    effective_size: f64,
    effective_usdc_size: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct ExposureSnapshot {
    path: PathBuf,
    age_secs: u64,
    positions_exposure_usdc: f64,
    open_order_exposure_usdc: f64,
    total_exposure_usdc: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct RiskEvaluation {
    max_total_exposure_usdc: Option<f64>,
    max_order_usdc: Option<f64>,
    order_usdc_size: f64,
    snapshot: Option<ExposureSnapshot>,
    projected_total_exposure_usdc: Option<f64>,
    blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveSubmitOutcome {
    order_id: String,
    status: String,
    success: bool,
    making_amount: String,
    taking_amount: String,
    error_msg: Option<String>,
    transaction_hashes: Vec<String>,
    trade_ids: Vec<String>,
    path: &'static str,
    payload_redacted: String,
    headers_redacted: String,
    metric_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LivePreviewOutcome {
    summary: String,
    path: &'static str,
    metric_lines: Vec<String>,
}

const SDK_SUBMIT_RETRY_COUNT: usize = 3;
const SDK_SUBMIT_RETRY_DELAY_MS: u64 = 750;

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
        "usage: run_copytrader_live_submit_gate [--root <path>] [--latest-activity <path>] [--selected-leader-env <path>] [--owner <value>] [--override-usdc-size <decimal>] [--max-total-exposure-usdc <decimal>] [--max-order-usdc <decimal>] [--account-snapshot <path>] [--account-snapshot-max-age-secs <n>] [--activity-max-age-secs <n>] [--order-type <GTC|GTD|FOK|FAK>] [--expiration-secs <n>] [--fee-rate-bps <n>] [--activity-source-verified] [--activity-under-budget] [--activity-capability-detected] [--positions-under-budget] [--allow-live-submit] [--force-live-submit]"
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
            "--owner" => options.owner = Some(next_value(&mut iter, arg)?),
            "--override-usdc-size" => {
                let value = next_value(&mut iter, arg)?;
                parse_decimal_value(&value)?;
                options.override_usdc_size = Some(value)
            }
            "--max-total-exposure-usdc" => {
                let value = next_value(&mut iter, arg)?;
                parse_decimal_value(&value)?;
                options.max_total_exposure_usdc = Some(value);
            }
            "--max-order-usdc" => {
                let value = next_value(&mut iter, arg)?;
                parse_decimal_value(&value)?;
                options.max_order_usdc = Some(value);
            }
            "--account-snapshot" => options.account_snapshot = Some(next_value(&mut iter, arg)?),
            "--account-snapshot-max-age-secs" => {
                options.account_snapshot_max_age_secs = parse_u64(
                    &next_value(&mut iter, arg)?,
                    "account-snapshot-max-age-secs",
                )?
            }
            "--activity-max-age-secs" => {
                options.activity_max_age_secs =
                    parse_u64(&next_value(&mut iter, arg)?, "activity-max-age-secs")?
            }
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
            "--force-live-submit" => options.force_live_submit = true,
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
    let gate_started_at_unix_ms = current_unix_ms()?;
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
    let execution_config = ExecutionAdapterConfig::from_root(&root).map_err(format_root_error)?;
    let material = auth_material_with_signer_fallback(&root)?;
    let draft = unsigned_order_from_activity(&latest, options)?;
    let risk = evaluate_risk_gate(&root, options, draft.effective_usdc_size)?;
    let activity_age_secs = current_unix_secs()?.saturating_sub(latest.timestamp);

    let derived_activity_source_verified = latest
        .wallet
        .as_deref()
        .is_some_and(|wallet| wallet.eq_ignore_ascii_case(&leader_wallet));
    let activity_source_verified = derived_activity_source_verified;
    let activity_source_verified_source = if derived_activity_source_verified {
        "derived_wallet_match"
    } else if latest.wallet.is_some() {
        "derived_wallet_mismatch"
    } else if options.activity_source_verified {
        "manual_flag_ignored_missing_wallet"
    } else {
        "missing"
    };

    let derived_activity_under_budget = activity_age_secs <= options.activity_max_age_secs;
    let activity_under_budget = derived_activity_under_budget;
    let activity_under_budget_source = if derived_activity_under_budget {
        "derived_activity_age"
    } else if options.activity_under_budget {
        "manual_flag_ignored_stale_activity"
    } else {
        "stale_or_missing"
    };

    let derived_activity_capability_detected = true;
    let activity_capability_detected = derived_activity_capability_detected;
    let activity_capability_detected_source = if options.activity_capability_detected {
        "manual_flag_ignored"
    } else {
        "derived_latest_activity_present"
    };

    let derived_positions_under_budget = risk.positions_under_budget_derived();
    let positions_under_budget = true;
    let positions_under_budget_source = if derived_positions_under_budget {
        "derived_account_snapshot_preview_ok"
    } else if risk.max_total_exposure_usdc.is_some() {
        "preview_allows_risk_gate_failure"
    } else if options.positions_under_budget {
        "manual_flag_ignored_preview_does_not_use_it"
    } else {
        "preview_not_risk_gated"
    };

    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.activity_source_verified = activity_source_verified;
    gate.activity_source_under_budget = activity_under_budget;
    gate.activity_capability_detected = activity_capability_detected;
    gate.positions_under_budget = positions_under_budget;
    gate.execution_surface_ready = execution_config.live_ready();
    let preview_blocked_reason = if options.force_live_submit {
        None
    } else {
        gate.blocked_reason()
    };
    let live_submit_readiness = preview_blocked_reason
        .clone()
        .or_else(|| {
            if options.force_live_submit {
                None
            } else {
                risk.blocked_reason.clone()
            }
        })
        .map(|reason| format!("blocked:{reason}"))
        .unwrap_or_else(|| "ready".to_string());
    let report_path = live_submit_report_path(&root)?;
    let auth_env_source = auth_env_source(&root);
    let auth_effective_funder_address = effective_funder_address(&material)
        .map_err(|error| format!("failed to resolve effective funder address: {error}"))?;
    let mut lines = vec![
        "mode=live-submit-gate".to_string(),
        format!("selected_leader_wallet={leader_wallet}"),
        format!("selected_leader_env_path={}", selected_leader_env.display()),
        format!("latest_activity_path={}", latest_activity_path.display()),
        format!("gate_started_at_unix_ms={gate_started_at_unix_ms}"),
        format!("auth_env_source={auth_env_source}"),
        format!("auth_signer_address={}", material.poly_address),
        format!(
            "auth_funder_address={}",
            material.funder.as_deref().unwrap_or("")
        ),
        format!(
            "auth_effective_funder_address={}",
            auth_effective_funder_address.as_deref().unwrap_or("")
        ),
        format!("auth_signature_type={}", material.signature_type),
        format!(
            "activity_wallet={}",
            latest.wallet.as_deref().unwrap_or("unknown")
        ),
        format!("activity_tx={}", latest.tx),
        format!("activity_timestamp={}", latest.timestamp),
        format!(
            "activity_timestamp_unix_ms={}",
            latest.timestamp.saturating_mul(1000)
        ),
        format!("activity_age_secs={activity_age_secs}"),
        format!("activity_max_age_secs={}", options.activity_max_age_secs),
        format!("activity_side={}", latest.side),
        format!(
            "activity_slug={}",
            latest.slug.as_deref().unwrap_or("unknown")
        ),
        format!("activity_asset={}", latest.asset),
        format!(
            "activity_price={}",
            latest
                .price
                .map(|value| format!("{value:.8}"))
                .unwrap_or_default()
        ),
        format!("order_size={:.6}", draft.effective_size),
        format!("order_usdc_size={:.6}", draft.effective_usdc_size),
        format!("unsigned_maker_amount={}", draft.unsigned.maker_amount),
        format!("unsigned_taker_amount={}", draft.unsigned.taker_amount),
        format!("gate_activity_source_verified={activity_source_verified}"),
        format!("gate_activity_source_verified_source={activity_source_verified_source}"),
        format!("gate_activity_under_budget={activity_under_budget}"),
        format!("gate_activity_under_budget_source={activity_under_budget_source}"),
        format!(
            "activity_age_over_budget_by_secs={}",
            activity_age_secs.saturating_sub(options.activity_max_age_secs)
        ),
        format!("gate_activity_capability_detected={activity_capability_detected}"),
        format!("gate_activity_capability_detected_source={activity_capability_detected_source}"),
        format!("gate_positions_under_budget={positions_under_budget}"),
        format!("gate_positions_under_budget_source={positions_under_budget_source}"),
        format!(
            "manual_gate_flags_present={}",
            options.activity_source_verified
                || options.activity_under_budget
                || options.activity_capability_detected
                || options.positions_under_budget
        ),
        format!("force_live_submit={}", options.force_live_submit),
        format!(
            "gate_execution_surface_ready={}",
            gate.execution_surface_ready
        ),
        format!(
            "preview_readiness={}",
            if preview_blocked_reason.is_some() {
                "blocked"
            } else {
                "ready"
            }
        ),
        format!("live_submit_readiness={live_submit_readiness}"),
        format!("risk_gate_status={}", risk.status_label()),
    ];

    if let Some(path) = &risk.snapshot {
        lines.push(format!(
            "risk_account_snapshot_path={}",
            path.path.display()
        ));
        lines.push(format!("risk_account_snapshot_age_secs={}", path.age_secs));
        lines.push(format!(
            "risk_positions_exposure_usdc={:.6}",
            path.positions_exposure_usdc
        ));
        lines.push(format!(
            "risk_open_order_exposure_usdc={:.6}",
            path.open_order_exposure_usdc
        ));
        lines.push(format!(
            "risk_current_total_exposure_usdc={:.6}",
            path.total_exposure_usdc
        ));
    }
    if let Some(value) = risk.max_total_exposure_usdc {
        lines.push(format!("risk_max_total_exposure_usdc={value:.6}"));
    }
    if let Some(value) = risk.max_order_usdc {
        lines.push(format!("risk_max_order_usdc={value:.6}"));
    }
    if let Some(value) = risk.projected_total_exposure_usdc {
        lines.push(format!("risk_projected_total_exposure_usdc={value:.6}"));
    }

    if let Some(reason) = preview_blocked_reason {
        lines.push(format!("live_gate_status=blocked:{reason}"));
        lines.push(format!("report_path={}", report_path.display()));
        write_report(&report_path, &lines)?;
        return Ok(lines);
    }

    lines.push("live_gate_status=unlocked".to_string());

    if options.allow_live_submit
        && !options.force_live_submit
        && let Some(reason) = &risk.blocked_reason
    {
        lines.push(format!("live_submit_status=blocked:{reason}"));
        lines.push(format!("report_path={}", report_path.display()));
        write_report(&report_path, &lines)?;
        return Ok(lines);
    }

    if !options.allow_live_submit {
        let sdk_base_url = execution_config
            .submit
            .base_url()
            .unwrap_or("https://clob.polymarket.com");
        match preview_live_order_with_sdk(sdk_base_url, &material, &latest, &draft, options) {
            Ok(preview) => {
                lines.push("live_submit_status=preview_only".to_string());
                lines.push(format!("preview_program={}", preview.path));
                lines.push("preview_args_redacted=true".to_string());
                lines.push(format!("preview_args={}", preview.summary));
                lines.extend(preview.metric_lines);
            }
            Err(error) => {
                lines.push("live_submit_status=preview_build_failed".to_string());
                lines.push(format!("preview_error={error}"));
            }
        }
        lines.push(format!("report_path={}", report_path.display()));
        write_report(&report_path, &lines)?;
        return Ok(lines);
    }

    let sdk_base_url = execution_config
        .submit
        .base_url()
        .unwrap_or("https://clob.polymarket.com");
    let result = submit_live_order_with_sdk(sdk_base_url, &material, &latest, &draft, options)?;

    lines.push("live_submit_status=submitted".to_string());
    lines.push(format!("submit_transport={}", result.path));
    lines.push(format!("submit_order_id={}", result.order_id));
    lines.push(format!("submit_order_status={}", result.status));
    lines.push(format!("submit_success={}", result.success));
    lines.push(format!("submit_making_amount={}", result.making_amount));
    lines.push(format!("submit_taking_amount={}", result.taking_amount));
    lines.push(format!(
        "submit_error_msg={}",
        result.error_msg.as_deref().unwrap_or("")
    ));
    lines.push("submit_method=POST".to_string());
    lines.push(format!("submit_url={sdk_base_url}/order"));
    lines.push(format!(
        "submit_payload_redacted={}",
        result.payload_redacted
    ));
    lines.push(format!(
        "submit_headers_redacted={}",
        result.headers_redacted
    ));
    lines.push(format!(
        "submit_transaction_hashes={}",
        result.transaction_hashes.join(",")
    ));
    lines.push(format!("submit_trade_ids={}", result.trade_ids.join(",")));
    lines.extend(result.metric_lines);
    lines.push(format!("report_path={}", report_path.display()));
    write_report(&report_path, &lines)?;
    Ok(lines)
}

fn unsigned_order_from_activity(
    latest: &LatestActivity,
    options: &Options,
) -> Result<PreparedOrderDraft, String> {
    let original_size = parse_decimal_value(&latest.size)?;
    let original_usdc_size = parse_decimal_value(&latest.usdc_size)?;
    let override_usdc_size = options
        .override_usdc_size
        .as_deref()
        .map(parse_decimal_value)
        .transpose()?;
    let (effective_usdc_size, effective_size) = if let Some(override_usdc_size) = override_usdc_size
    {
        if original_usdc_size <= 0.0 {
            return Err("cannot override usdc size when latest activity usdcSize <= 0".to_string());
        }
        let ratio = override_usdc_size / original_usdc_size;
        (override_usdc_size, original_size * ratio)
    } else {
        (original_usdc_size, original_size)
    };
    let size = fixed_6_from_f64(effective_size)?;
    let usdc_size = fixed_6_from_f64(effective_usdc_size)?;
    let side = latest.side.to_uppercase();
    let (maker_amount, taker_amount) = match side.as_str() {
        "BUY" => (usdc_size.clone(), size.clone()),
        "SELL" => (size.clone(), usdc_size.clone()),
        other => return Err(format!("unsupported activity side: {other}")),
    };

    Ok(PreparedOrderDraft {
        unsigned: UnsignedOrderPayload {
            taker: "0x0000000000000000000000000000000000000000".into(),
            token_id: latest.asset.clone(),
            maker_amount,
            taker_amount,
            side,
            expiration: (latest.timestamp + options.expiration_secs).to_string(),
            nonce: latest.timestamp.to_string(),
            fee_rate_bps: options.fee_rate_bps.to_string(),
        },
        effective_size,
        effective_usdc_size,
    })
}

fn parse_decimal_value(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid decimal value: {value}"))
}

fn fixed_6_from_f64(value: f64) -> Result<String, String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!("invalid non-negative decimal value: {value}"));
    }
    decimal_to_fixed_6(&format!("{value:.6}"))
}

fn decimal_to_fixed_6(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("empty decimal value".to_string());
    }
    let negative = trimmed.starts_with('-');
    let trimmed = trimmed.trim_start_matches('-');
    let (whole, frac) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    let whole = whole.chars().filter(|ch| *ch != ',').collect::<String>();
    if !whole.chars().all(|ch| ch.is_ascii_digit()) || !frac.chars().all(|ch| ch.is_ascii_digit()) {
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

impl RiskEvaluation {
    fn status_label(&self) -> String {
        match &self.blocked_reason {
            Some(reason) => format!("blocked:{reason}"),
            None if self.max_total_exposure_usdc.is_some() || self.max_order_usdc.is_some() => {
                "ready".to_string()
            }
            None => "not_configured".to_string(),
        }
    }

    fn positions_under_budget_derived(&self) -> bool {
        self.max_total_exposure_usdc.is_some()
            && self.blocked_reason.is_none()
            && self.snapshot.is_some()
    }
}

fn evaluate_risk_gate(
    root: &Path,
    options: &Options,
    order_usdc_size: f64,
) -> Result<RiskEvaluation, String> {
    let max_total_exposure_usdc = options
        .max_total_exposure_usdc
        .as_deref()
        .map(parse_decimal_value)
        .transpose()?;
    let max_order_usdc = options
        .max_order_usdc
        .as_deref()
        .map(parse_decimal_value)
        .transpose()?;

    let mut blocked_reason = None;
    if let Some(cap) = max_order_usdc
        && order_usdc_size > cap
    {
        blocked_reason = Some("order_notional_exceeds_cap".to_string());
    }

    let require_snapshot = max_total_exposure_usdc.is_some()
        || (options.allow_live_submit && max_order_usdc.is_some());
    let snapshot = if require_snapshot {
        match resolve_account_snapshot_path(root, options) {
            Some(path) => match (file_age_secs(&path), read_account_exposure_snapshot(&path)) {
                (Ok(age_secs), Ok(exposure)) => {
                    if age_secs > options.account_snapshot_max_age_secs {
                        blocked_reason.get_or_insert_with(|| "account_snapshot_stale".to_string());
                    }
                    Some(ExposureSnapshot {
                        path,
                        age_secs,
                        positions_exposure_usdc: exposure.0,
                        open_order_exposure_usdc: exposure.1,
                        total_exposure_usdc: exposure.2,
                    })
                }
                _ => {
                    blocked_reason.get_or_insert_with(|| "account_snapshot_unreadable".to_string());
                    None
                }
            },
            None => {
                blocked_reason.get_or_insert_with(|| "account_snapshot_missing".to_string());
                None
            }
        }
    } else {
        None
    };

    if options.allow_live_submit && max_total_exposure_usdc.is_none() {
        blocked_reason.get_or_insert_with(|| "missing_total_exposure_cap".to_string());
    }

    let projected_total_exposure_usdc = snapshot
        .as_ref()
        .map(|snapshot| snapshot.total_exposure_usdc + order_usdc_size);
    if let (Some(cap), Some(projected_total)) =
        (max_total_exposure_usdc, projected_total_exposure_usdc)
        && projected_total > cap
    {
        blocked_reason.get_or_insert_with(|| "total_exposure_would_exceed_cap".to_string());
    }

    Ok(RiskEvaluation {
        max_total_exposure_usdc,
        max_order_usdc,
        order_usdc_size,
        snapshot,
        projected_total_exposure_usdc,
        blocked_reason,
    })
}

fn submit_live_order_with_sdk(
    base_url: &str,
    material: &AuthMaterial,
    latest: &LatestActivity,
    draft: &PreparedOrderDraft,
    options: &Options,
) -> Result<LiveSubmitOutcome, String> {
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build tokio runtime: {error}"))?;
    let mut last_error = None;
    for attempt in 1..=SDK_SUBMIT_RETRY_COUNT {
        match runtime.block_on(submit_live_order_with_sdk_async(
            base_url, material, latest, draft, options,
        )) {
            Ok(mut outcome) => {
                if attempt > 1 {
                    outcome.path = "rust_sdk_retry";
                }
                return Ok(outcome);
            }
            Err(error) => {
                let retryable = sdk_submit_error_retryable(&error);
                if !retryable || attempt == SDK_SUBMIT_RETRY_COUNT {
                    return Err(match last_error {
                        Some(previous) => {
                            format!("{error} (retry_history={previous}; final_attempt={attempt})")
                        }
                        None => error,
                    });
                }
                last_error = Some(error);
                thread::sleep(Duration::from_millis(SDK_SUBMIT_RETRY_DELAY_MS));
            }
        }
    }
    Err("sdk submit retry loop exited unexpectedly".to_string())
}

fn preview_live_order_with_sdk(
    base_url: &str,
    material: &AuthMaterial,
    latest: &LatestActivity,
    draft: &PreparedOrderDraft,
    options: &Options,
) -> Result<LivePreviewOutcome, String> {
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build tokio runtime: {error}"))?;
    runtime.block_on(preview_live_order_with_sdk_async(
        base_url, material, latest, draft, options,
    ))
}

async fn preview_live_order_with_sdk_async(
    base_url: &str,
    material: &AuthMaterial,
    latest: &LatestActivity,
    draft: &PreparedOrderDraft,
    options: &Options,
) -> Result<LivePreviewOutcome, String> {
    let payload_build_started_at_unix_ms = current_unix_ms()?;
    let signer = LocalSigner::from_str(&material.private_key)
        .map_err(|error| format!("invalid private key for sdk preview: {error}"))?
        .with_chain_id(Some(POLYGON));

    let mut auth_builder = SdkClobClient::new(
        base_url,
        SdkClobConfig::builder().use_server_time(true).build(),
    )
    .map_err(|error| format!("failed to build sdk client: {error}"))?
    .authentication_builder(&signer);

    if let Some(credentials) = sdk_credentials_from_material(material)? {
        auth_builder = auth_builder.credentials(credentials);
    }

    let signature_type = sdk_signature_type(material.signature_type)?;
    if signature_type != SdkSignatureType::Eoa {
        auth_builder = auth_builder.signature_type(signature_type);
    }
    if let Some(funder) = material
        .funder
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let address = SdkAddress::from_str(funder)
            .map_err(|error| format!("invalid FUNDER_ADDRESS: {error}"))?;
        auth_builder = auth_builder.funder(address);
    }

    let client = auth_builder
        .authenticate()
        .await
        .map_err(|error| format!("sdk authenticate failed: {error}"))?;

    let token_id = latest
        .asset
        .parse()
        .map_err(|error| format!("invalid activity asset token id {}: {error}", latest.asset))?;
    let side = sdk_side_from_activity(&latest.side)?;
    let order_type = sdk_order_type(options.order_type);
    let signable_order = match side {
        SdkSide::Buy => client
            .market_order()
            .token_id(token_id)
            .amount(
                SdkAmount::usdc(sdk_usdc_amount(draft.effective_usdc_size)?)
                    .map_err(|error| format!("invalid usdc amount for sdk preview: {error}"))?,
            )
            .side(SdkSide::Buy)
            .order_type(order_type.clone())
            .build()
            .await
            .map_err(|error| format!("sdk market order preview build failed: {error}"))?,
        SdkSide::Sell => client
            .market_order()
            .token_id(token_id)
            .amount(
                SdkAmount::shares(sdk_share_amount(draft.effective_size)?)
                    .map_err(|error| format!("invalid share amount for sdk preview: {error}"))?,
            )
            .side(SdkSide::Sell)
            .order_type(order_type.clone())
            .build()
            .await
            .map_err(|error| format!("sdk market order preview build failed: {error}"))?,
        _ => {
            return Err(format!(
                "unsupported activity side for sdk preview: {}",
                latest.side
            ));
        }
    };
    let order_built_at_unix_ms = current_unix_ms()?;
    let follower_effective_price = effective_price_from_signable_order(&signable_order)?;
    let signed_order = client
        .sign(&signer, signable_order)
        .await
        .map_err(|error| format!("sdk preview sign failed: {error}"))?;
    let payload_ready_at_unix_ms = current_unix_ms()?;

    let amount_label = match side {
        SdkSide::Buy => format!("usdc={:.6}", draft.effective_usdc_size),
        SdkSide::Sell => format!("shares={:.6}", draft.effective_size),
        _ => unreachable!(),
    };
    let mut metric_lines = build_latency_and_price_lines(
        latest,
        payload_build_started_at_unix_ms,
        Some(order_built_at_unix_ms),
        Some(payload_ready_at_unix_ms),
        None,
        None,
        follower_effective_price,
    );
    metric_lines.push(format!(
        "preview_payload_redacted={}",
        render_redacted_signed_order_payload(&signed_order)
    ));

    Ok(LivePreviewOutcome {
        summary: format!(
            "rust_sdk_preview side={} token_id={} order_type={} {} signature_type={} funder={}",
            latest.side.to_ascii_uppercase(),
            latest.asset,
            order_type,
            amount_label,
            material.signature_type,
            material.funder.as_deref().unwrap_or("")
        ),
        path: "rust_sdk",
        metric_lines,
    })
}

async fn submit_live_order_with_sdk_async(
    base_url: &str,
    material: &AuthMaterial,
    latest: &LatestActivity,
    draft: &PreparedOrderDraft,
    options: &Options,
) -> Result<LiveSubmitOutcome, String> {
    let payload_build_started_at_unix_ms = current_unix_ms()?;
    let signer = LocalSigner::from_str(&material.private_key)
        .map_err(|error| format!("invalid private key for sdk submit: {error}"))?
        .with_chain_id(Some(POLYGON));

    let mut auth_builder = SdkClobClient::new(
        base_url,
        SdkClobConfig::builder().use_server_time(true).build(),
    )
    .map_err(|error| format!("failed to build sdk client: {error}"))?
    .authentication_builder(&signer);

    if let Some(credentials) = sdk_credentials_from_material(material)? {
        auth_builder = auth_builder.credentials(credentials);
    }

    let signature_type = sdk_signature_type(material.signature_type)?;
    if signature_type != SdkSignatureType::Eoa {
        auth_builder = auth_builder.signature_type(signature_type);
    }
    if let Some(funder) = material
        .funder
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let address = SdkAddress::from_str(funder)
            .map_err(|error| format!("invalid FUNDER_ADDRESS: {error}"))?;
        auth_builder = auth_builder.funder(address);
    }

    let client = auth_builder
        .authenticate()
        .await
        .map_err(|error| format!("sdk authenticate failed: {error}"))?;

    let token_id = latest
        .asset
        .parse()
        .map_err(|error| format!("invalid activity asset token id {}: {error}", latest.asset))?;
    let side = sdk_side_from_activity(&latest.side)?;
    let order_type = sdk_order_type(options.order_type);
    let signable_order = match side {
        SdkSide::Buy => client
            .market_order()
            .token_id(token_id)
            .amount(
                SdkAmount::usdc(sdk_usdc_amount(draft.effective_usdc_size)?)
                    .map_err(|error| format!("invalid usdc amount for sdk submit: {error}"))?,
            )
            .side(SdkSide::Buy)
            .order_type(order_type)
            .build()
            .await
            .map_err(|error| format!("sdk market order build failed: {error}"))?,
        SdkSide::Sell => client
            .market_order()
            .token_id(token_id)
            .amount(
                SdkAmount::shares(sdk_share_amount(draft.effective_size)?)
                    .map_err(|error| format!("invalid share amount for sdk submit: {error}"))?,
            )
            .side(SdkSide::Sell)
            .order_type(order_type)
            .build()
            .await
            .map_err(|error| format!("sdk market order build failed: {error}"))?,
        SdkSide::Unknown => {
            return Err(format!(
                "unsupported activity side for sdk submit: {}",
                latest.side
            ));
        }
        _ => {
            return Err(format!(
                "unsupported activity side for sdk submit: {}",
                latest.side
            ));
        }
    };
    let order_built_at_unix_ms = current_unix_ms()?;
    let follower_effective_price = effective_price_from_signable_order(&signable_order)?;

    let signed_order = client
        .sign(&signer, signable_order)
        .await
        .map_err(|error| format!("sdk sign failed: {error}"))?;
    let payload_ready_at_unix_ms = current_unix_ms()?;
    let payload_redacted = render_redacted_signed_order_payload(&signed_order);
    let headers_redacted = render_redacted_submit_headers(material, &signer.address().to_string());
    let submit_started_at_unix_ms = current_unix_ms()?;
    let response = client
        .post_order(signed_order)
        .await
        .map_err(|error| {
            format!(
                "sdk post_order failed: {error}; submit_method=POST; submit_url={base_url}/order; payload_redacted={payload_redacted}; headers_redacted={headers_redacted}"
            )
        })?;
    let submit_finished_at_unix_ms = current_unix_ms()?;
    let metric_lines = build_latency_and_price_lines(
        latest,
        payload_build_started_at_unix_ms,
        Some(order_built_at_unix_ms),
        Some(payload_ready_at_unix_ms),
        Some(submit_started_at_unix_ms),
        Some(submit_finished_at_unix_ms),
        follower_effective_price,
    );

    Ok(LiveSubmitOutcome {
        order_id: response.order_id,
        status: response.status.to_string(),
        success: response.success,
        making_amount: response.making_amount.to_string(),
        taking_amount: response.taking_amount.to_string(),
        error_msg: response.error_msg.filter(|value| !value.trim().is_empty()),
        transaction_hashes: response
            .transaction_hashes
            .into_iter()
            .map(|hash| hash.to_string())
            .collect(),
        trade_ids: response.trade_ids,
        path: "rust_sdk",
        payload_redacted,
        headers_redacted,
        metric_lines,
    })
}

fn sdk_credentials_from_material(
    material: &AuthMaterial,
) -> Result<Option<SdkCredentials>, String> {
    let Some(secret) = material
        .api_secret
        .clone()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let api_key = Uuid::parse_str(&material.api_key)
        .map_err(|error| format!("invalid CLOB_API_KEY for sdk credentials: {error}"))?;
    Ok(Some(SdkCredentials::new(
        api_key,
        secret,
        material.passphrase.clone(),
    )))
}

fn sdk_signature_type(value: u8) -> Result<SdkSignatureType, String> {
    match value {
        0 => Ok(SdkSignatureType::Eoa),
        1 => Ok(SdkSignatureType::Proxy),
        2 => Ok(SdkSignatureType::GnosisSafe),
        other => Err(format!(
            "unsupported SIGNATURE_TYPE for sdk submit: {other}"
        )),
    }
}

fn sdk_side_from_activity(side: &str) -> Result<SdkSide, String> {
    match side.to_ascii_uppercase().as_str() {
        "BUY" => Ok(SdkSide::Buy),
        "SELL" => Ok(SdkSide::Sell),
        other => Err(format!("unsupported activity side: {other}")),
    }
}

fn sdk_submit_error_retryable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("425 too early")
        || lower.contains("service not ready")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection reset")
        || lower.contains("temporarily unavailable")
}

fn build_latency_and_price_lines(
    latest: &LatestActivity,
    payload_build_started_at_unix_ms: u64,
    order_built_at_unix_ms: Option<u64>,
    payload_ready_at_unix_ms: Option<u64>,
    submit_started_at_unix_ms: Option<u64>,
    submit_finished_at_unix_ms: Option<u64>,
    follower_effective_price: f64,
) -> Vec<String> {
    let leader_ts_ms = latest.timestamp.saturating_mul(1000);
    let mut lines = vec![
        format!(
            "payload_build_started_at_unix_ms={payload_build_started_at_unix_ms}"
        ),
        format!("follower_effective_price={follower_effective_price:.8}"),
    ];

    if let Some(leader_price) = latest.price {
        let gap = follower_effective_price - leader_price;
        let gap_bps = if leader_price.abs() > f64::EPSILON {
            (gap / leader_price) * 10_000.0
        } else {
            0.0
        };
        let adverse_gap_bps = if latest.side.eq_ignore_ascii_case("BUY") {
            if gap_bps > 0.0 { gap_bps } else { 0.0 }
        } else if gap_bps < 0.0 {
            -gap_bps
        } else {
            0.0
        };
        lines.push(format!("leader_price={leader_price:.8}"));
        lines.push(format!("price_gap={gap:.8}"));
        lines.push(format!("price_gap_bps={gap_bps:.4}"));
        lines.push(format!("adverse_price_gap_bps={adverse_gap_bps:.4}"));
    }

    if let Some(order_built) = order_built_at_unix_ms {
        lines.push(format!("order_built_at_unix_ms={order_built}"));
        lines.push(format!(
            "order_build_elapsed_ms={}",
            order_built.saturating_sub(payload_build_started_at_unix_ms)
        ));
    }
    if let Some(payload_ready) = payload_ready_at_unix_ms {
        lines.push(format!("payload_ready_at_unix_ms={payload_ready}"));
        lines.push(format!(
            "payload_prep_elapsed_ms={}",
            payload_ready.saturating_sub(payload_build_started_at_unix_ms)
        ));
        lines.push(format!(
            "leader_to_payload_ready_ms={}",
            payload_ready.saturating_sub(leader_ts_ms)
        ));
    }
    if let Some(submit_started) = submit_started_at_unix_ms {
        lines.push(format!("submit_started_at_unix_ms={submit_started}"));
        lines.push(format!(
            "leader_to_submit_started_ms={}",
            submit_started.saturating_sub(leader_ts_ms)
        ));
    }
    if let (Some(submit_started), Some(submit_finished)) =
        (submit_started_at_unix_ms, submit_finished_at_unix_ms)
    {
        lines.push(format!("submit_finished_at_unix_ms={submit_finished}"));
        lines.push(format!(
            "submit_roundtrip_elapsed_ms={}",
            submit_finished.saturating_sub(submit_started)
        ));
        lines.push(format!(
            "leader_to_submit_finished_ms={}",
            submit_finished.saturating_sub(leader_ts_ms)
        ));
    }

    lines
}

fn effective_price_from_signable_order(order: &SdkSignableOrder) -> Result<f64, String> {
    let maker = order
        .order
        .makerAmount
        .to_string()
        .parse::<f64>()
        .map_err(|error| format!("invalid makerAmount while computing effective price: {error}"))?
        / 1_000_000.0;
    let taker = order
        .order
        .takerAmount
        .to_string()
        .parse::<f64>()
        .map_err(|error| format!("invalid takerAmount while computing effective price: {error}"))?
        / 1_000_000.0;
    if maker <= 0.0 || taker <= 0.0 {
        return Err("non-positive order amounts while computing effective price".to_string());
    }
    match order.order.side {
        0 => Ok(maker / taker),
        1 => Ok(taker / maker),
        other => Err(format!(
            "unsupported order side while computing effective price: {other}"
        )),
    }
}

fn effective_funder_address(material: &AuthMaterial) -> Result<Option<String>, String> {
    if let Some(funder) = material
        .funder
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Some(funder.to_string()));
    }

    let signer = SdkAddress::from_str(&material.poly_address).map_err(|error| {
        format!("invalid signer address for effective funder derivation: {error}")
    })?;

    let derived = match material.signature_type {
        0 => None,
        1 => derive_proxy_wallet(signer, POLYGON),
        2 => derive_safe_wallet(signer, POLYGON),
        other => {
            return Err(format!(
                "unsupported SIGNATURE_TYPE for effective funder derivation: {other}"
            ));
        }
    };

    Ok(derived.map(|address| address.to_string()))
}

fn render_redacted_signed_order_payload(order: &SdkSignedOrder) -> String {
    let side = match order.order.side {
        0 => "BUY",
        1 => "SELL",
        _ => "UNKNOWN",
    };
    let owner = redact_token(&order.owner.to_string(), 8, 4);
    let signature = redact_token(&order.signature.to_string(), 10, 6);
    let post_only = match order.post_only {
        Some(value) => value.to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"order\":{{\"salt\":\"{}\",\"maker\":\"{}\",\"signer\":\"{}\",\"taker\":\"{}\",\"tokenId\":\"{}\",\"makerAmount\":\"{}\",\"takerAmount\":\"{}\",\"expiration\":\"{}\",\"nonce\":\"{}\",\"feeRateBps\":\"{}\",\"side\":\"{}\",\"signatureType\":{},\"signature\":\"{}\"}},\"orderType\":\"{}\",\"owner\":\"{}\",\"postOnly\":{}}}",
        order.order.salt,
        order.order.maker,
        order.order.signer,
        order.order.taker,
        order.order.tokenId,
        order.order.makerAmount,
        order.order.takerAmount,
        order.order.expiration,
        order.order.nonce,
        order.order.feeRateBps,
        side,
        order.order.signatureType,
        signature,
        order.order_type,
        owner,
        post_only
    )
}

fn render_redacted_submit_headers(material: &AuthMaterial, signer_address: &str) -> String {
    let api_key = redact_token(&material.api_key, 8, 4);
    let passphrase = format!("[redacted:{}]", material.passphrase.len());
    format!(
        "POLY_ADDRESS={};POLY_API_KEY={};POLY_PASSPHRASE={};POLY_SIGNATURE=[derived_hmac_from_secret_and_request];POLY_TIMESTAMP=[server_time];SIGNATURE_TYPE={};FUNDER_ADDRESS={}",
        signer_address,
        api_key,
        passphrase,
        material.signature_type,
        material.funder.as_deref().unwrap_or("")
    )
}

fn redact_token(value: &str, prefix: usize, suffix: usize) -> String {
    if value.len() <= prefix + suffix {
        return "[redacted]".to_string();
    }
    format!(
        "{}...{}",
        &value[..prefix.min(value.len())],
        &value[value.len() - suffix.min(value.len())..]
    )
}

fn sdk_order_type(order_type: OrderType) -> SdkOrderType {
    match order_type {
        OrderType::Gtc => SdkOrderType::GTC,
        OrderType::Gtd => SdkOrderType::GTD,
        OrderType::Fok => SdkOrderType::FOK,
        OrderType::Fak => SdkOrderType::FAK,
    }
}

fn sdk_usdc_amount(value: f64) -> Result<SdkDecimal, String> {
    sdk_decimal_from_scaled_f64(value, 6, "order_usdc_size")
}

fn sdk_share_amount(value: f64) -> Result<SdkDecimal, String> {
    let truncated = (value * 100.0).floor() / 100.0;
    if truncated <= 0.0 {
        return Err(format!(
            "order_size rounds to zero at market-share precision: {value:.6}"
        ));
    }
    sdk_decimal_from_scaled_f64(truncated, 2, "order_size")
}

fn sdk_decimal_from_scaled_f64(
    value: f64,
    scale: usize,
    field: &str,
) -> Result<SdkDecimal, String> {
    let normalized = normalize_non_negative_zero(value);
    if !normalized.is_finite() || normalized <= 0.0 {
        return Err(format!("invalid positive decimal for {field}: {value}"));
    }
    let formatted = format!("{normalized:.scale$}");
    SdkDecimal::from_str(&formatted)
        .map_err(|error| format!("invalid sdk decimal for {field} ({formatted}): {error}"))
}

fn resolve_account_snapshot_path(root: &Path, options: &Options) -> Option<PathBuf> {
    options
        .account_snapshot
        .as_ref()
        .map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .or_else(|| {
            [
                root.join("runtime-verify-account/dashboard.json"),
                root.join("dashboard.json"),
            ]
            .into_iter()
            .find(|path| path.exists())
        })
}

fn file_age_secs(path: &Path) -> Result<u64, String> {
    let modified = fs::metadata(path)
        .map_err(|error| format!("failed to read metadata for {}: {error}", path.display()))?
        .modified()
        .map_err(|error| {
            format!(
                "failed to read modified time for {}: {error}",
                path.display()
            )
        })?;
    let now = SystemTime::now();
    now.duration_since(modified)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error for {}: {error}", path.display()))
}

fn read_account_exposure_snapshot(path: &Path) -> Result<(f64, f64, f64), String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let snapshot = extract_nested_object(&body, "account_snapshot")
        .or_else(|| first_json_object(&body))
        .ok_or_else(|| {
            format!(
                "failed to parse account snapshot JSON from {}",
                path.display()
            )
        })?;
    let positions = extract_json_array(&snapshot, "positions").unwrap_or_else(|| "[]".to_string());
    let open_orders =
        extract_json_array(&snapshot, "open_orders").unwrap_or_else(|| "[]".to_string());

    let positions_exposure_usdc = iter_json_objects(&positions)
        .into_iter()
        .filter_map(|object| {
            extract_field_value(&object, "estimated_equity")
                .or_else(|| extract_field_value(&object, "currentValue"))
        })
        .map(|value| parse_decimal_value(&value))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(f64::abs)
        .sum::<f64>();

    let open_order_exposure_usdc = iter_json_objects(&open_orders)
        .into_iter()
        .map(|object| open_order_notional_usdc(&object))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<f64>();

    Ok((
        normalize_non_negative_zero(positions_exposure_usdc),
        normalize_non_negative_zero(open_order_exposure_usdc),
        normalize_non_negative_zero(positions_exposure_usdc + open_order_exposure_usdc),
    ))
}

fn open_order_notional_usdc(object: &str) -> Result<f64, String> {
    if let Some(value) = extract_field_value(object, "notional") {
        return parse_decimal_value(&value).map(f64::abs);
    }
    let price = extract_field_value(object, "price")
        .as_deref()
        .map(parse_decimal_value)
        .transpose()?
        .unwrap_or_default();
    let size = extract_field_value(object, "original_size")
        .or_else(|| extract_field_value(object, "size"))
        .as_deref()
        .map(parse_decimal_value)
        .transpose()?
        .unwrap_or_default();
    Ok((price * size).abs())
}

fn extract_nested_object(content: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let start = content.find(&needle)?;
    let rest = &content[start + needle.len()..];
    let brace_index = rest.find('{')? + start + needle.len();
    object_bounds(content, brace_index).map(|(from, to)| content[from..=to].to_string())
}

fn extract_json_array(content: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let start = content.find(&needle)?;
    let rest = &content[start + needle.len()..];
    let bracket_index = rest.find('[')? + start + needle.len();
    array_bounds(content, bracket_index).map(|(from, to)| content[from..=to].to_string())
}

fn array_bounds(content: &str, anchor: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let start = content[..=anchor].rfind('[')?;
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
            b'[' if !in_string => depth += 1,
            b']' if !in_string => {
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

fn iter_json_objects(content: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = content[cursor..].find('{') {
        let start = cursor + offset;
        if let Some((from, to)) = object_bounds(content, start) {
            objects.push(content[from..=to].to_string());
            cursor = to + 1;
        } else {
            break;
        }
    }
    objects
}

fn current_unix_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error: {error}"))
}

fn current_unix_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| format!("system time error: {error}"))
}

fn normalize_non_negative_zero(value: f64) -> f64 {
    if value.abs() < 1e-9 { 0.0 } else { value }
}

fn auth_material_with_signer_fallback(root: &Path) -> Result<AuthMaterial, String> {
    match AuthMaterial::from_root(root) {
        Ok(material) => Ok(material),
        Err(RootEnvLoadError::MissingField(field)) if field == "POLY_ADDRESS" => {
            let env_map = merged_env(root)?;
            let poly_address = derive_signer_address_from_private_key(&env_map)?;
            let api_key = required_env(&env_map, &["CLOB_API_KEY", "POLY_API_KEY"])?;
            let passphrase = required_env(&env_map, &["CLOB_PASS_PHRASE", "POLY_PASSPHRASE"])?;
            let private_key = required_env(&env_map, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"])?;
            let signature_type = optional_env(&env_map, &["SIGNATURE_TYPE"])
                .unwrap_or_else(|| "0".to_string())
                .parse::<u8>()
                .map_err(|_| "invalid SIGNATURE_TYPE".to_string())?;
            let funder = optional_env(&env_map, &["FUNDER_ADDRESS", "FUNDER"]);
            let api_secret = optional_env(&env_map, &["CLOB_SECRET", "POLY_API_SECRET"]);
            let mut material = AuthMaterial::new(
                poly_address,
                api_key,
                passphrase,
                private_key,
                signature_type,
                funder,
            );
            if let Some(api_secret) = api_secret {
                material = material.with_api_secret(api_secret);
            }
            Ok(material)
        }
        Err(error) => Err(format_root_error(error)),
    }
}

fn derive_signer_address_from_private_key(
    env_map: &BTreeMap<String, String>,
) -> Result<String, String> {
    let private_key = required_env(env_map, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"])?;
    let signer = LocalSigner::from_str(&private_key)
        .map_err(|error| format!("failed to derive signer from private key: {error}"))?
        .with_chain_id(Some(POLYGON));
    Ok(signer.address().to_string())
}

fn env_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u64(name: &str, default: u64) -> u64 {
    env_value(name)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn merged_env(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut env_map = env::vars().collect::<BTreeMap<_, _>>();
    let env_path = root.join(".env");
    let env_local_path = root.join(".env.local");
    if env_path.exists() {
        merge_env_file(&mut env_map, &env_path)?;
    } else {
        merge_env_file(&mut env_map, &env_local_path)?;
    }
    clear_proxy_env(&mut env_map);
    Ok(env_map)
}

fn merge_env_file(env_map: &mut BTreeMap<String, String>, path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env_map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(())
}

fn required_env(env_map: &BTreeMap<String, String>, keys: &[&str]) -> Result<String, String> {
    optional_env(env_map, keys).ok_or_else(|| format!("missing field {}", keys[0]))
}

fn optional_env(env_map: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env_map.get(*key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn clear_proxy_env(env_map: &mut BTreeMap<String, String>) {
    for key in [
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        env_map.insert(key.to_string(), String::new());
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
                    (!value.is_empty() && is_valid_evm_wallet(value)).then(|| value.to_string())
                }
                _ => None,
            }
        })
        .ok_or_else(|| format!("missing valid leader wallet in {}", path.display()))
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
            .map_err(|error| format!("invalid latest activity timestamp: {error}"))?,
        price: extract_field_value(&object, "price")
            .as_deref()
            .map(parse_decimal_value)
            .transpose()?,
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

fn auth_env_source(root: &Path) -> &'static str {
    if root.join(".env").exists() {
        ".env"
    } else if root.join(".env.local").exists() {
        ".env.local"
    } else {
        "process_env_only"
    }
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
    use super::{
        LatestActivity, Options, auth_material_with_signer_fallback, decimal_to_fixed_6,
        effective_funder_address, evaluate_risk_gate, parse_args, read_latest_activity,
        read_selected_leader_wallet, sdk_share_amount, sdk_submit_error_retryable,
        unsigned_order_from_activity,
    };
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
            "--max-total-exposure-usdc".into(),
            "100".into(),
            "--max-order-usdc".into(),
            "10".into(),
            "--account-snapshot".into(),
            "runtime-verify-account/dashboard.json".into(),
            "--allow-live-submit".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert!(options.activity_source_verified);
        assert!(options.activity_under_budget);
        assert!(options.activity_capability_detected);
        assert!(options.positions_under_budget);
        assert_eq!(options.max_total_exposure_usdc.as_deref(), Some("100"));
        assert_eq!(options.max_order_usdc.as_deref(), Some("10"));
        assert_eq!(
            options.account_snapshot.as_deref(),
            Some("runtime-verify-account/dashboard.json")
        );
        assert!(options.allow_live_submit);
    }

    #[test]
    fn decimal_to_fixed_6_converts_decimal_strings_to_micros() {
        assert_eq!(decimal_to_fixed_6("1.23").expect("micros"), "1230000");
        assert_eq!(decimal_to_fixed_6("0.000001").expect("micros"), "1");
        assert_eq!(decimal_to_fixed_6("12").expect("micros"), "12000000");
    }

    #[test]
    fn read_latest_activity_extracts_price_asset_and_sizes() {
        let root = unique_temp_dir("latest");
        fs::create_dir_all(&root).expect("temp dir created");
        let latest = root.join("latest.json");
        fs::write(
            &latest,
            r#"[{"proxyWallet":"0x11084005d88A0840b5F38F8731CCa9152BbD99F7","transactionHash":"0xabc","timestamp":1776303488,"price":0.4575,"side":"BUY","slug":"market-a","asset":"12345","size":138.6735,"usdcSize":63.45}]"#,
        )
        .expect("latest written");

        let activity = read_latest_activity(&latest).expect("activity");
        assert_eq!(
            activity.wallet.as_deref(),
            Some("0x11084005d88A0840b5F38F8731CCa9152BbD99F7")
        );
        assert_eq!(activity.price, Some(0.4575));
        assert_eq!(activity.asset, "12345");
        assert_eq!(activity.size, "138.6735");
        assert_eq!(activity.usdc_size, "63.45");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn unsigned_order_from_activity_swaps_amounts_for_sell_side() {
        let event = LatestActivity {
            wallet: Some("0x11084005d88A0840b5F38F8731CCa9152BbD99F7".into()),
            tx: "0xabc".into(),
            timestamp: 1_776_303_488,
            price: Some(0.5),
            side: "SELL".into(),
            slug: Some("market-a".into()),
            asset: "12345".into(),
            size: "10".into(),
            usdc_size: "5".into(),
        };
        let order = unsigned_order_from_activity(&event, &Options::default())
            .expect("order")
            .unsigned;

        assert_eq!(order.token_id, "12345");
        assert_eq!(order.side, "SELL");
        assert_eq!(order.maker_amount, "10000000");
        assert_eq!(order.taker_amount, "5000000");
    }

    #[test]
    fn unsigned_order_from_activity_scales_amounts_from_override_usdc_size() {
        let event = LatestActivity {
            wallet: Some("0x11084005d88A0840b5F38F8731CCa9152BbD99F7".into()),
            tx: "0xabc".into(),
            timestamp: 1_776_303_488,
            price: Some(0.5),
            side: "BUY".into(),
            slug: Some("market-a".into()),
            asset: "asset-1".into(),
            size: "10".into(),
            usdc_size: "5".into(),
        };
        let options = Options {
            override_usdc_size: Some("2.5".into()),
            ..Options::default()
        };

        let draft = unsigned_order_from_activity(&event, &options).expect("order");

        assert_eq!(draft.unsigned.maker_amount, "2500000");
        assert_eq!(draft.unsigned.taker_amount, "5000000");
        assert_eq!(draft.effective_usdc_size, 2.5);
    }

    #[test]
    fn read_selected_leader_wallet_rejects_placeholder_wallet() {
        let root = unique_temp_dir("invalid-wallet");
        fs::create_dir_all(&root).expect("temp dir created");
        let env_path = root.join("selected-leader.env");
        fs::write(&env_path, "COPYTRADER_DISCOVERY_WALLET=0xleader\n").expect("env written");

        let error = read_selected_leader_wallet(&env_path).expect_err("wallet should fail");
        assert!(error.contains("missing valid leader wallet"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn evaluate_risk_gate_reads_account_snapshot_and_blocks_excess_total_exposure() {
        let root = unique_temp_dir("risk-gate");
        fs::create_dir_all(root.join("runtime-verify-account")).expect("snapshot dir created");
        fs::write(
            root.join("runtime-verify-account/dashboard.json"),
            r#"{
  "account_snapshot": {
    "open_orders": [{"price":"0.50","original_size":"8"}],
    "positions": [{"estimated_equity":"25.5"},{"estimated_equity":"-4.5"}]
  }
}"#,
        )
        .expect("snapshot written");

        let options = Options {
            root: root.display().to_string(),
            max_total_exposure_usdc: Some("30".into()),
            max_order_usdc: Some("10".into()),
            allow_live_submit: true,
            ..Options::default()
        };

        let risk = evaluate_risk_gate(&root, &options, 6.0).expect("risk");
        assert_eq!(
            risk.blocked_reason.as_deref(),
            Some("total_exposure_would_exceed_cap")
        );
        assert!(!risk.positions_under_budget_derived());
        assert_eq!(
            risk.snapshot
                .as_ref()
                .map(|snapshot| snapshot.total_exposure_usdc),
            Some(34.0)
        );
        assert_eq!(risk.projected_total_exposure_usdc, Some(40.0));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn sdk_share_amount_truncates_to_market_lot_precision() {
        let amount = sdk_share_amount(3.456789).expect("share amount");
        assert_eq!(amount.to_string(), "3.45");
    }

    #[test]
    fn auth_material_with_signer_fallback_derives_signer_from_private_key() {
        let root = unique_temp_dir("auth-fallback");
        fs::create_dir_all(&root).expect("root dir created");
        fs::write(
            root.join(".env"),
            concat!(
                "CLOB_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80\n",
                "CLOB_API_KEY=123e4567-e89b-12d3-a456-426614174000\n",
                "CLOB_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n",
                "CLOB_PASS_PHRASE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
                "SIGNATURE_TYPE=1\n",
                "FUNDER_ADDRESS=0x0bdc847347571342e1563971e8ba206c8b03e345\n",
            ),
        )
        .expect("env written");

        let material = auth_material_with_signer_fallback(&root).expect("auth material");
        assert!(
            material
                .poly_address
                .eq_ignore_ascii_case("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
        assert_eq!(material.signature_type, 1);
        assert_eq!(
            material.funder.as_deref(),
            Some("0x0bdc847347571342e1563971e8ba206c8b03e345")
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn auth_material_with_signer_fallback_prefers_env_over_env_local() {
        let root = unique_temp_dir("auth-env-precedence");
        fs::create_dir_all(&root).expect("root dir created");
        fs::write(
            root.join(".env"),
            concat!(
                "CLOB_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80\n",
                "CLOB_API_KEY=123e4567-e89b-12d3-a456-426614174000\n",
                "CLOB_PASS_PHRASE=env-passphrase\n",
                "SIGNATURE_TYPE=1\n",
            ),
        )
        .expect(".env written");
        fs::write(
            root.join(".env.local"),
            concat!(
                "CLOB_PRIVATE_KEY=0x59c6995e998f97a5a0044976f2e84b3b76c3f7d28d2e58c5b7a4d4a8b0d5f4f4\n",
                "FUNDER_ADDRESS=0xfunder-address\n",
                "SIGNATURE_TYPE=2\n",
            ),
        )
        .expect(".env.local written");

        let material = auth_material_with_signer_fallback(&root).expect("auth material");
        assert!(
            material
                .poly_address
                .eq_ignore_ascii_case("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
        assert_eq!(material.signature_type, 1);
        assert_eq!(material.funder, None);

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn sdk_submit_error_retryable_matches_service_not_ready_and_timeouts() {
        assert!(sdk_submit_error_retryable(
            "sdk post_order failed: Status: error(425 Too Early) making POST call to /order with service not ready"
        ));
        assert!(sdk_submit_error_retryable(
            "sdk post_order failed: request timed out while connecting"
        ));
        assert!(!sdk_submit_error_retryable(
            "sdk post_order failed: invalid signature"
        ));
    }

    #[test]
    fn effective_funder_address_derives_proxy_wallet_when_missing() {
        let root = unique_temp_dir("effective-funder");
        fs::create_dir_all(&root).expect("root dir created");
        fs::write(
            root.join(".env"),
            concat!(
                "CLOB_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80\n",
                "CLOB_API_KEY=123e4567-e89b-12d3-a456-426614174000\n",
                "CLOB_PASS_PHRASE=env-passphrase\n",
                "SIGNATURE_TYPE=1\n",
            ),
        )
        .expect(".env written");

        let material = auth_material_with_signer_fallback(&root).expect("auth material");
        let effective = effective_funder_address(&material).expect("effective funder");
        assert!(effective.is_some());
        let effective = effective.expect("derived funder");
        assert!(effective.starts_with("0x"));
        assert_eq!(effective.len(), 42);

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
