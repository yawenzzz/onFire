use futures::StreamExt as _;
use polymarket_client_sdk::auth::{Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid};
use polymarket_client_sdk::clob::ws::{Client as WsClient, WsMessage};
use polymarket_client_sdk::types::Address as SdkAddress;
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
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    json: bool,
    max_events: Option<usize>,
    output: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            json: false,
            max_events: None,
            output: None,
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
        "usage: run_copytrader_account_ws [--root <path>] [--json] [--max-events <n>] [--output <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--json" => options.json = true,
            "--max-events" => {
                options.max_events = Some(
                    next_value(&mut iter, arg)?
                        .parse::<usize>()
                        .map_err(|_| "invalid integer for max-events".to_string())?
                        .max(1),
                )
            }
            "--output" => options.output = Some(next_value(&mut iter, arg)?),
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
    let material = auth_material_with_signer_fallback(&root)?;
    let credentials = sdk_credentials_from_material(&material)?
        .ok_or_else(|| "missing CLOB_SECRET for websocket auth".to_string())?;
    let effective_funder =
        effective_funder_address(&material)?.unwrap_or_else(|| material.poly_address.clone());
    let address = SdkAddress::from_str(&effective_funder)
        .map_err(|error| format!("invalid websocket auth address: {error}"))?;
    let client = WsClient::default()
        .authenticate(credentials, address)
        .map_err(|error| format!("ws authenticate failed: {error}"))?;
    let mut stream = Box::pin(
        client
            .subscribe_user_events(Vec::new())
            .map_err(|error| format!("subscribe_user_events failed: {error}"))?,
    );

    let output_path = options.output.as_ref().map(|path| {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    });

    let mut seen = 0usize;
    while let Some(event) = stream.next().await {
        let event = event.map_err(|error| format!("user ws stream error: {error}"))?;
        let payload = match event {
            WsMessage::Order(order) => render_ws_payload(
                &root,
                &material,
                &effective_funder,
                json!({
                    "type": "order",
                    "id": order.id,
                    "market": order.market.to_string(),
                    "asset_id": order.asset_id.to_string(),
                    "side": order.side.to_string(),
                    "price": order.price.to_string(),
                    "status": order.status.map(|value| value.to_string()),
                    "msg_type": order.msg_type.map(|value| format!("{value:?}")),
                    "original_size": order.original_size.map(|value| value.to_string()),
                    "size_matched": order.size_matched.map(|value| value.to_string()),
                    "timestamp": order.timestamp,
                    "outcome": order.outcome,
                }),
            )?,
            WsMessage::Trade(trade) => render_ws_payload(
                &root,
                &material,
                &effective_funder,
                json!({
                    "type": "trade",
                    "id": trade.id,
                    "market": trade.market.to_string(),
                    "asset_id": trade.asset_id.to_string(),
                    "side": trade.side.to_string(),
                    "price": trade.price.to_string(),
                    "size": trade.size.to_string(),
                    "status": format!("{:?}", trade.status),
                    "transaction_hash": trade.transaction_hash.map(|value| value.to_string()),
                    "timestamp": trade.timestamp,
                    "matchtime": trade.matchtime,
                    "last_update": trade.last_update,
                    "trader_side": trade.trader_side.map(|value| format!("{value:?}")),
                    "outcome": trade.outcome,
                }),
            )?,
            other => render_ws_payload(
                &root,
                &material,
                &effective_funder,
                json!({
                    "type": "other",
                    "raw": format!("{other:?}")
                }),
            )?,
        };

        let rendered = if options.json {
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to render websocket payload json: {error}"))?
        } else {
            payload["event"]["type"]
                .as_str()
                .unwrap_or("other")
                .to_string()
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

        seen += 1;
        if let Some(max_events) = options.max_events
            && seen >= max_events
        {
            return Ok(());
        }
    }

    Ok(())
}

fn render_ws_payload(
    root: &Path,
    material: &AuthMaterial,
    effective_funder: &str,
    event: Value,
) -> Result<Value, String> {
    Ok(json!({
        "updated_at_unix": current_unix_secs()?,
        "account_status": {
            "mode": "account-ws-live",
            "auth_env_source": auth_env_source(root),
            "signer_address": material.poly_address,
            "effective_funder_address": effective_funder,
            "signature_type": material.signature_type,
        },
        "event": event,
    }))
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

fn auth_env_source(root: &Path) -> &'static str {
    if root.join(".env").exists() {
        ".env"
    } else if root.join(".env.local").exists() {
        ".env.local"
    } else {
        "process_env_only"
    }
}

fn current_unix_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error: {error}"))
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
    use super::render_ws_payload;
    use rust_copytrader::adapters::signing::AuthMaterial;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn ws_payload_contains_account_status_and_event_shape() {
        let material = AuthMaterial::new(
            "0xC7694b1771B95e95bFD53eCCD4A55EB5a1658D77",
            "00000000-0000-0000-0000-000000000001",
            "passphrase",
            "0x1234",
            1,
            None,
        )
        .with_api_secret("secret");

        let payload = render_ws_payload(
            Path::new("."),
            &material,
            "0x0bdc847347571342e1563971e8ba206c8b03e345",
            json!({
                "type": "trade",
                "id": "trade-1",
                "market": "market-1",
                "transaction_hash": "0xabc",
            }),
        )
        .unwrap();

        assert_eq!(payload["account_status"]["mode"], "account-ws-live");
        assert_eq!(payload["account_status"]["signature_type"], 1);
        assert_eq!(payload["event"]["type"], "trade");
        assert_eq!(payload["event"]["transaction_hash"], "0xabc");
    }
}
