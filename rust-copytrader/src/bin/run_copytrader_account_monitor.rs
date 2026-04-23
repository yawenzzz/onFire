use polymarket_client_sdk::auth::{Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid};
use polymarket_client_sdk::clob::types::request::{
    BalanceAllowanceRequest, OrdersRequest, TradesRequest, UpdateBalanceAllowanceRequest,
};
use polymarket_client_sdk::clob::types::{
    AssetType, Side as SdkSide, SignatureType as SdkSignatureType, TradeStatusType,
};
use polymarket_client_sdk::clob::{Client as SdkClobClient, Config as SdkClobConfig};
use polymarket_client_sdk::data::Client as SdkDataClient;
use polymarket_client_sdk::data::types::ActivityType as DataActivityType;
use polymarket_client_sdk::data::types::request::{
    ActivityRequest, ClosedPositionsRequest, ValueRequest,
};
use polymarket_client_sdk::data::types::response::{
    Activity as DataActivity, ClosedPosition as DataClosedPosition, Value as DataValueResponse,
};
use polymarket_client_sdk::types::{Address as SdkAddress, Decimal as SdkDecimal};
use polymarket_client_sdk::{POLYGON, derive_proxy_wallet, derive_safe_wallet};
use rust_copytrader::adapters::signing::AuthMaterial;
use rust_copytrader::config::RootEnvLoadError;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    json: bool,
    watch: bool,
    interval_secs: u64,
    output: Option<String>,
    max_iterations: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
struct DataParitySnapshot {
    public_data_status: String,
    activity_status: String,
    closed_positions_status: String,
    value_status: String,
    activities: Vec<Value>,
    cash_history: Vec<Value>,
    closed_positions: Vec<Value>,
    public_value_records: Vec<Value>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            json: false,
            watch: false,
            interval_secs: 5,
            output: None,
            max_iterations: None,
        }
    }
}

#[tokio::main]
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

    match run(&options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!(
        "usage: run_copytrader_account_monitor [--root <path>] [--json] [--watch] [--interval-secs <n>] [--output <path>] [--max-iterations <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--json" => options.json = true,
            "--watch" => options.watch = true,
            "--interval-secs" => {
                options.interval_secs = next_value(&mut iter, arg)?
                    .parse::<u64>()
                    .map_err(|_| "invalid integer for interval-secs".to_string())?
                    .max(1)
            }
            "--output" => options.output = Some(next_value(&mut iter, arg)?),
            "--max-iterations" => {
                options.max_iterations = Some(
                    next_value(&mut iter, arg)?
                        .parse::<usize>()
                        .map_err(|_| "invalid integer for max-iterations".to_string())?
                        .max(1),
                )
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

async fn run(options: &Options) -> Result<(), String> {
    let root = PathBuf::from(&options.root);
    let output_path = options.output.as_ref().map(|path| {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    });

    let mut iteration = 0usize;
    loop {
        iteration += 1;
        let snapshot = build_snapshot(&root).await?;
        let rendered = if options.json {
            serde_json::to_string_pretty(&snapshot)
                .map_err(|error| format!("failed to render account snapshot json: {error}"))?
        } else {
            render_summary(&snapshot)
        };

        if let Some(path) = &output_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            }
            fs::write(path, &rendered)
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        }

        println!("{rendered}");

        if !options.watch {
            return Ok(());
        }
        if let Some(max_iterations) = options.max_iterations
            && iteration >= max_iterations
        {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(options.interval_secs));
    }
}

async fn build_snapshot(root: &Path) -> Result<Value, String> {
    let material = auth_material_with_signer_fallback(root)?;
    let signer = LocalSigner::from_str(&material.private_key)
        .map_err(|error| format!("invalid private key for account monitor: {error}"))?
        .with_chain_id(Some(POLYGON));

    let mut auth_builder = SdkClobClient::new(
        "https://clob.polymarket.com",
        SdkClobConfig::builder().use_server_time(true).build(),
    )
    .map_err(|error| format!("failed to build sdk client: {error}"))?
    .authentication_builder(&signer);

    if let Some(credentials) = sdk_credentials_from_material(&material)? {
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
        auth_builder = auth_builder.funder(
            SdkAddress::from_str(funder)
                .map_err(|error| format!("invalid FUNDER_ADDRESS: {error}"))?,
        );
    }

    let client = auth_builder
        .authenticate()
        .await
        .map_err(|error| format!("sdk authenticate failed: {error}"))?;

    let _ = client
        .update_balance_allowance(
            UpdateBalanceAllowanceRequest::builder()
                .asset_type(AssetType::Collateral)
                .build(),
        )
        .await;
    let balance = client
        .balance_allowance(
            BalanceAllowanceRequest::builder()
                .asset_type(AssetType::Collateral)
                .build(),
        )
        .await
        .map_err(|error| format!("balance_allowance failed: {error}"))?;
    let orders = client
        .orders(&OrdersRequest::default(), None)
        .await
        .map_err(|error| format!("orders failed: {error}"))?;
    let trades = client
        .trades(&TradesRequest::default(), None)
        .await
        .map_err(|error| format!("trades failed: {error}"))?;

    let effective_funder = effective_funder_address(&material)?;
    let user_address = SdkAddress::from_str(
        effective_funder
            .as_deref()
            .unwrap_or(&material.poly_address),
    )
    .map_err(|error| format!("invalid account monitor user address: {error}"))?;
    let data_parity = fetch_data_parity(user_address).await;

    let positions = derive_positions(&trades.data);
    let fees_paid = trades
        .data
        .iter()
        .map(|trade| (trade.size * trade.price * trade.fee_rate_bps) / SdkDecimal::from(10_000))
        .fold(SdkDecimal::ZERO, |acc, value| acc + value);
    let net_cash_flow = trades.data.iter().fold(SdkDecimal::ZERO, |acc, trade| {
        let notional = trade.size * trade.price;
        match trade.side {
            SdkSide::Buy => acc - notional,
            SdkSide::Sell => acc + notional,
            _ => acc,
        }
    });
    let positions_equity = positions.iter().fold(SdkDecimal::ZERO, |acc, position| {
        acc + SdkDecimal::from_str(position["estimated_equity"].as_str().unwrap_or("0"))
            .unwrap_or(SdkDecimal::ZERO)
    });
    let estimated_equity = positions_equity + balance.balance;
    let estimated_total_pnl = estimated_equity + net_cash_flow - fees_paid;
    let activities_count = data_parity.activities.len();
    let cash_history_count = data_parity.cash_history.len();
    let closed_positions_count = data_parity.closed_positions.len();
    let activities = data_parity.activities;
    let cash_history = data_parity.cash_history;
    let closed_positions = data_parity.closed_positions;
    let public_value_records = data_parity.public_value_records;
    let public_data_status = data_parity.public_data_status;
    let activity_status = data_parity.activity_status;
    let closed_positions_status = data_parity.closed_positions_status;
    let value_status = data_parity.value_status;

    let account_snapshot = json!({
        "balances": {
            "balance": balance.balance.to_string(),
            "allowances": balance.allowances.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<BTreeMap<_, _>>(),
        },
        "open_orders_count": orders.count,
        "open_orders": orders.data.iter().map(|order| json!({
            "id": order.id,
            "status": order.status.to_string(),
            "market": order.market.to_string(),
            "asset_id": order.asset_id.to_string(),
            "side": order.side.to_string(),
            "original_size": order.original_size.to_string(),
            "size_matched": order.size_matched.to_string(),
            "price": order.price.to_string(),
            "outcome": order.outcome,
            "order_type": order.order_type.to_string(),
        })).collect::<Vec<_>>(),
        "recent_trades_count": trades.count,
        "recent_trades": trades.data.iter().map(|trade| json!({
            "id": trade.id,
            "asset_id": trade.asset_id.to_string(),
            "market": trade.market.to_string(),
            "side": trade.side.to_string(),
            "size": trade.size.to_string(),
            "price": trade.price.to_string(),
            "status": trade.status.to_string(),
            "transaction_hash": trade.transaction_hash.to_string(),
            "match_time": trade.match_time.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "activities_count": activities_count,
        "activities": activities,
        "cash_history_count": cash_history_count,
        "cash_history": cash_history,
        "closed_positions_count": closed_positions_count,
        "closed_positions": closed_positions,
        "public_value_records": public_value_records,
        "public_data_status": public_data_status,
        "activity_status": activity_status,
        "closed_positions_status": closed_positions_status,
        "value_status": value_status,
        "positions": positions,
        "pnl_summary": {
            "fees_paid": fees_paid.to_string(),
            "net_cash_flow": net_cash_flow.to_string(),
            "estimated_equity": estimated_equity.to_string(),
            "estimated_total_pnl": estimated_total_pnl.to_string(),
            "open_position_count": positions.len(),
            "pnl_source": "trade_mark_to_last_trade"
        }
    });

    render_account_monitor_payload(root, &material, effective_funder, account_snapshot)
}

fn derive_positions(
    trades: &[polymarket_client_sdk::clob::types::response::TradeResponse],
) -> Vec<Value> {
    let mut by_asset = BTreeMap::<String, (SdkDecimal, SdkDecimal)>::new();
    for trade in trades {
        let entry = by_asset
            .entry(trade.asset_id.to_string())
            .or_insert((SdkDecimal::ZERO, trade.price));
        match trade.side {
            SdkSide::Buy => entry.0 += trade.size,
            SdkSide::Sell => entry.0 -= trade.size,
            _ => {}
        }
        if matches!(
            trade.status,
            TradeStatusType::Matched | TradeStatusType::Mined | TradeStatusType::Confirmed
        ) {
            entry.1 = trade.price;
        }
    }
    by_asset
        .into_iter()
        .filter(|(_, (net_size, _))| !net_size.is_zero())
        .map(|(asset_id, (net_size, last_price))| {
            let equity = net_size * last_price;
            json!({
                "asset_id": asset_id,
                "net_size": net_size.to_string(),
                "last_price": last_price.to_string(),
                "estimated_equity": equity.to_string(),
            })
        })
        .collect()
}

async fn fetch_data_parity(user: SdkAddress) -> DataParitySnapshot {
    let client = SdkDataClient::default();
    let activities_request = ActivityRequest::builder()
        .user(user)
        .limit(50)
        .expect("activity limit 50 must be valid")
        .build();
    let closed_positions_request = ClosedPositionsRequest::builder()
        .user(user)
        .limit(50)
        .expect("closed positions limit 50 must be valid")
        .build();
    let value_request = ValueRequest::builder().user(user).build();

    let activities_result = client.activity(&activities_request).await;

    let closed_positions_result = client.closed_positions(&closed_positions_request).await;

    let value_result = client.value(&value_request).await;

    let activities = activities_result
        .as_ref()
        .map(|records| {
            records
                .iter()
                .map(render_activity_record)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let cash_history = activities_result
        .as_ref()
        .map(|records| {
            records
                .iter()
                .filter(|record| is_cash_history_activity(record))
                .map(render_activity_record)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let closed_positions = closed_positions_result
        .as_ref()
        .map(|records| {
            records
                .iter()
                .map(render_closed_position_record)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let public_value_records = value_result
        .as_ref()
        .map(|records| records.iter().map(render_value_record).collect::<Vec<_>>())
        .unwrap_or_default();

    let activity_status = activities_result
        .as_ref()
        .map(|_| "ok".to_string())
        .unwrap_or_else(|error| format!("error:{error}"));
    let closed_positions_status = closed_positions_result
        .as_ref()
        .map(|_| "ok".to_string())
        .unwrap_or_else(|error| format!("error:{error}"));
    let value_status = value_result
        .as_ref()
        .map(|_| "ok".to_string())
        .unwrap_or_else(|error| format!("error:{error}"));

    let public_data_status =
        if activities_result.is_ok() && closed_positions_result.is_ok() && value_result.is_ok() {
            "ok".to_string()
        } else if activities_result.is_err()
            && closed_positions_result.is_err()
            && value_result.is_err()
        {
            "error".to_string()
        } else {
            "partial".to_string()
        };

    DataParitySnapshot {
        public_data_status,
        activity_status,
        closed_positions_status,
        value_status,
        activities,
        cash_history,
        closed_positions,
        public_value_records,
    }
}

fn render_activity_record(activity: &DataActivity) -> Value {
    json!({
        "proxy_wallet": activity.proxy_wallet.to_string(),
        "timestamp": activity.timestamp,
        "condition_id": activity.condition_id.map(|value| value.to_string()),
        "activity_type": activity.activity_type.to_string(),
        "size": activity.size.to_string(),
        "usdc_size": activity.usdc_size.to_string(),
        "transaction_hash": activity.transaction_hash.to_string(),
        "price": activity.price.map(|value| value.to_string()),
        "asset": activity.asset.map(|value| value.to_string()),
        "side": activity.side.as_ref().map(ToString::to_string),
        "outcome_index": activity.outcome_index,
        "title": activity.title,
        "slug": activity.slug,
        "event_slug": activity.event_slug,
        "outcome": activity.outcome,
    })
}

fn render_closed_position_record(position: &DataClosedPosition) -> Value {
    json!({
        "proxy_wallet": position.proxy_wallet.to_string(),
        "asset": position.asset.to_string(),
        "condition_id": position.condition_id.to_string(),
        "avg_price": position.avg_price.to_string(),
        "total_bought": position.total_bought.to_string(),
        "realized_pnl": position.realized_pnl.to_string(),
        "cur_price": position.cur_price.to_string(),
        "timestamp": position.timestamp,
        "title": position.title,
        "slug": position.slug,
        "event_slug": position.event_slug,
        "outcome": position.outcome,
    })
}

fn render_value_record(value: &DataValueResponse) -> Value {
    json!({
        "user": value.user.to_string(),
        "value": value.value.to_string(),
    })
}

fn is_cash_history_activity(activity: &DataActivity) -> bool {
    !matches!(activity.activity_type, DataActivityType::Trade)
}

fn render_account_monitor_payload(
    root: &Path,
    material: &AuthMaterial,
    effective_funder: Option<String>,
    account_snapshot: Value,
) -> Result<Value, String> {
    Ok(json!({
        "updated_at_unix": current_unix_secs()?,
        "account_status": {
            "mode": "account-live",
            "auth_env_source": auth_env_source(root),
            "signer_address": material.poly_address,
            "funder_address": material.funder,
            "effective_funder_address": effective_funder,
            "signature_type": material.signature_type,
            "reason": "clob account api connected"
        },
        "account_snapshot": account_snapshot,
    }))
}

fn render_summary(snapshot: &Value) -> String {
    let status = &snapshot["account_status"];
    let account = &snapshot["account_snapshot"];
    [
        format!("mode={}", status["mode"].as_str().unwrap_or("unknown")),
        format!(
            "auth_env_source={}",
            status["auth_env_source"].as_str().unwrap_or("")
        ),
        format!(
            "signer_address={}",
            status["signer_address"].as_str().unwrap_or("")
        ),
        format!(
            "effective_funder_address={}",
            status["effective_funder_address"].as_str().unwrap_or("")
        ),
        format!(
            "balance={}",
            account["balances"]["balance"].as_str().unwrap_or("0")
        ),
        format!(
            "open_orders_count={}",
            account["open_orders_count"].as_u64().unwrap_or(0)
        ),
        format!(
            "recent_trades_count={}",
            account["recent_trades_count"].as_u64().unwrap_or(0)
        ),
        format!(
            "open_position_count={}",
            account["pnl_summary"]["open_position_count"]
                .as_u64()
                .unwrap_or(0)
        ),
        format!(
            "estimated_equity={}",
            account["pnl_summary"]["estimated_equity"]
                .as_str()
                .unwrap_or("0")
        ),
        format!(
            "estimated_total_pnl={}",
            account["pnl_summary"]["estimated_total_pnl"]
                .as_str()
                .unwrap_or("0")
        ),
    ]
    .join("\n")
}

fn current_unix_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error: {error}"))
}

fn auth_material_with_signer_fallback(root: &Path) -> Result<AuthMaterial, String> {
    match AuthMaterial::from_root(root) {
        Ok(material) => Ok(material),
        Err(RootEnvLoadError::MissingField(field)) if field == "POLY_ADDRESS" => {
            let env_map = merged_env(root)?;
            let signer = LocalSigner::from_str(
                env_map
                    .get("CLOB_PRIVATE_KEY")
                    .or_else(|| env_map.get("PRIVATE_KEY"))
                    .ok_or_else(|| "missing private key".to_string())?,
            )
            .map_err(|error| format!("failed to derive signer from private key: {error}"))?
            .with_chain_id(Some(POLYGON));
            let mut env_map = env_map;
            env_map.insert("POLY_ADDRESS".into(), signer.address().to_string());
            env_map.insert("SIGNER_ADDRESS".into(), signer.address().to_string());
            AuthMaterial::from_env_map(&env_map).map_err(format_root_error)
        }
        Err(error) => Err(format_root_error(error)),
    }
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
        other => Err(format!("unsupported SIGNATURE_TYPE: {other}")),
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

fn auth_env_source(root: &Path) -> &'static str {
    if root.join(".env").exists() {
        ".env"
    } else if root.join(".env.local").exists() {
        ".env.local"
    } else {
        "process_env_only"
    }
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
        is_cash_history_activity, render_account_monitor_payload, render_activity_record,
        render_closed_position_record, render_value_record,
    };
    use polymarket_client_sdk::data::types::ActivityType as DataActivityType;
    use polymarket_client_sdk::data::types::Side as DataSide;
    use polymarket_client_sdk::data::types::response::{
        Activity as DataActivity, ClosedPosition as DataClosedPosition, Value as DataValueResponse,
    };
    use polymarket_client_sdk::types::{
        Address as SdkAddress, B256, Decimal as SdkDecimal, U256, Utc,
    };
    use rust_copytrader::adapters::signing::AuthMaterial;
    use serde_json::json;
    use std::path::Path;
    use std::str::FromStr as _;

    fn sample_activity(activity_type: DataActivityType) -> DataActivity {
        DataActivity::builder()
            .proxy_wallet(
                SdkAddress::from_str("0x0bdc847347571342e1563971e8ba206c8b03e345").unwrap(),
            )
            .timestamp(1_714_000_000)
            .condition_id(
                B256::from_str(
                    "0x1111111111111111111111111111111111111111111111111111111111111111",
                )
                .unwrap(),
            )
            .activity_type(activity_type)
            .size(SdkDecimal::from_str("1.5").unwrap())
            .usdc_size(SdkDecimal::from_str("2.5").unwrap())
            .transaction_hash(
                B256::from_str(
                    "0x2222222222222222222222222222222222222222222222222222222222222222",
                )
                .unwrap(),
            )
            .price(SdkDecimal::from_str("0.55").unwrap())
            .asset(U256::from(42))
            .side(DataSide::Buy)
            .outcome_index(0)
            .title("market".to_string())
            .slug("market-slug".to_string())
            .event_slug("event-slug".to_string())
            .outcome("YES".to_string())
            .build()
    }

    #[test]
    fn render_activity_record_contains_expected_schema_fields() {
        let rendered = render_activity_record(&sample_activity(DataActivityType::Trade));
        assert_eq!(rendered["activity_type"], "TRADE");
        assert_eq!(
            rendered["transaction_hash"],
            "0x2222222222222222222222222222222222222222222222222222222222222222"
        );
        assert_eq!(rendered["usdc_size"], "2.5");
        assert_eq!(rendered["slug"], "market-slug");
        assert_eq!(rendered["side"], "BUY");
    }

    #[test]
    fn cash_history_filters_non_trade_activity_types() {
        assert!(!is_cash_history_activity(&sample_activity(
            DataActivityType::Trade
        )));
        assert!(is_cash_history_activity(&sample_activity(
            DataActivityType::Redeem
        )));
        assert!(is_cash_history_activity(&sample_activity(
            DataActivityType::Reward
        )));
    }

    #[test]
    fn render_closed_position_record_contains_expected_schema_fields() {
        let position = DataClosedPosition::builder()
            .proxy_wallet(
                SdkAddress::from_str("0x0bdc847347571342e1563971e8ba206c8b03e345").unwrap(),
            )
            .asset(U256::from(7))
            .condition_id(
                B256::from_str(
                    "0x3333333333333333333333333333333333333333333333333333333333333333",
                )
                .unwrap(),
            )
            .avg_price(SdkDecimal::from_str("0.42").unwrap())
            .total_bought(SdkDecimal::from_str("10").unwrap())
            .realized_pnl(SdkDecimal::from_str("1.2").unwrap())
            .cur_price(SdkDecimal::from_str("0.51").unwrap())
            .timestamp(1_714_000_111)
            .title("closed market".to_string())
            .slug("closed-slug".to_string())
            .icon("icon".to_string())
            .event_slug("closed-event".to_string())
            .outcome("NO".to_string())
            .outcome_index(1)
            .opposite_outcome("YES".to_string())
            .opposite_asset(U256::from(8))
            .end_date(Utc::now())
            .build();

        let rendered = render_closed_position_record(&position);
        assert_eq!(rendered["realized_pnl"], "1.2");
        assert_eq!(rendered["slug"], "closed-slug");
        assert_eq!(rendered["outcome"], "NO");
    }

    #[test]
    fn render_value_record_contains_expected_schema_fields() {
        let value = DataValueResponse::builder()
            .user(SdkAddress::from_str("0x0bdc847347571342e1563971e8ba206c8b03e345").unwrap())
            .value(SdkDecimal::from_str("99").unwrap())
            .build();
        let rendered = render_value_record(&value);
        assert_eq!(rendered["value"], "99");
        assert_eq!(
            rendered["user"].as_str().unwrap().to_ascii_lowercase(),
            "0x0bdc847347571342e1563971e8ba206c8b03e345"
        );
    }

    #[test]
    fn account_monitor_payload_includes_history_keys() {
        let material = AuthMaterial::new(
            "0xC7694b1771B95e95bFD53eCCD4A55EB5a1658D77",
            "00000000-0000-0000-0000-000000000001",
            "passphrase",
            "0x1234",
            1,
            None,
        )
        .with_api_secret("secret");
        let payload = render_account_monitor_payload(
            Path::new("."),
            &material,
            Some("0x0bdc847347571342e1563971e8ba206c8b03e345".to_string()),
            json!({
                "activities": [json!({"activity_type": "REDEEM"})],
                "cash_history": [json!({"activity_type": "REDEEM"})],
                "public_value_records": [json!({"value": "99"})],
                "closed_positions": [json!({"realized_pnl": "1.2"})],
            }),
        )
        .unwrap();

        assert!(payload["account_snapshot"]["activities"].is_array());
        assert!(payload["account_snapshot"]["cash_history"].is_array());
        assert!(payload["account_snapshot"]["public_value_records"].is_array());
        assert!(payload["account_snapshot"]["closed_positions"].is_array());
        assert_eq!(
            payload["account_status"]["effective_funder_address"],
            "0x0bdc847347571342e1563971e8ba206c8b03e345"
        );
    }
}
