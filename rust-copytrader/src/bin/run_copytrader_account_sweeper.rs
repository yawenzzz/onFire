use alloy::primitives::Bytes as AlloyBytes;
use alloy::providers::ProviderBuilder;
use alloy::signers::Signer as _;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use alloy::sol_types::SolCall;
use polymarket_client_sdk::POLYGON;
use polymarket_client_sdk::auth::LocalSigner;
use polymarket_client_sdk::ctf::Client as CtfClient;
use polymarket_client_sdk::ctf::types::{
    MergePositionsRequest, RedeemNegRiskRequest, RedeemPositionsRequest,
};
use polymarket_client_sdk::data::Client as SdkDataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_client_sdk::data::types::response::Position as DataPosition;
use polymarket_client_sdk::types::{Address as SdkAddress, B256, Decimal as SdkDecimal, U256};
use rust_copytrader::adapters::signing::AuthMaterial;
use rust_copytrader::config::RootEnvLoadError;
use std::cmp::min;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use tokio::time::{Duration, sleep};

const DEFAULT_RPC_URL: &str = "https://polygon.drpc.org";
const DEFAULT_PUBLIC_RPC_URLS: &[&str] = &[
    "https://polygon.drpc.org",
    "https://tenderly.rpc.polygon.community",
    "https://polygon.publicnode.com",
    "https://polygon-public.nodies.app",
    "https://1rpc.io/matic",
    "https://polygon.api.onfinality.io/public",
];
const POLYGON_PUSD: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
const POLYGON_CTF: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const POLYGON_NEG_RISK_ADAPTER: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";
const POLYGON_PROXY_FACTORY: &str = "0xaB45c5A4B0c941a2F231C04C3f49182e1A254052";
const DEFAULT_INTERVAL_SECS: u64 = 30;

sol! {
    struct ProxyCall {
        uint8 typeCode;
        address to;
        uint256 value;
        bytes data;
    }

    #[sol(rpc)]
    interface IProxyWalletFactory {
        function proxy(ProxyCall[] calls) external payable returns (bytes[] returnValues);
    }

    #[sol(rpc)]
    interface IERC1155Lite {
        function setApprovalForAll(address operator, bool approved) external;
        function isApprovedForAll(address account, address operator) external view returns (bool);
    }

    interface IConditionalTokensLite {
        function redeemPositions(address collateralToken, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets) external;
        function mergePositions(address collateralToken, bytes32 parentCollectionId, bytes32 conditionId, uint256[] partition, uint256 amount) external;
    }

    interface INegRiskAdapterLite {
        function redeemPositions(bytes32 conditionId, uint256[] amounts) external;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    watch: bool,
    interval_secs: u64,
    max_iterations: Option<usize>,
    positions_limit: i32,
    allow_live_submit: bool,
    execution_mode: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            watch: false,
            interval_secs: DEFAULT_INTERVAL_SECS,
            max_iterations: None,
            positions_limit: 500,
            allow_live_submit: false,
            execution_mode: "auto".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionFilter {
    Mergeable,
    Redeemable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalletKind {
    Eoa,
    Proxy,
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionTransport {
    DirectRpc,
    RelayerApi,
    BuilderApi,
    ClobL2Hook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionProfile {
    wallet_kind: WalletKind,
    transport: ExecutionTransport,
    signature_type: u8,
    has_clob_l2_auth: bool,
    has_relayer_api_auth: bool,
    has_builder_api_auth: bool,
    signer_address: Option<String>,
    effective_account_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MergeCandidate {
    condition_id: B256,
    slug: String,
    event_slug: String,
    negative_risk: bool,
    yes_micros: u128,
    no_micros: u128,
    merge_micros: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedeemCandidate {
    condition_id: B256,
    slug: String,
    event_slug: String,
    negative_risk: bool,
    yes_micros: u128,
    no_micros: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SweepPlan {
    merge_positions_scanned: usize,
    redeem_positions_scanned: usize,
    merge_candidates: Vec<MergeCandidate>,
    redeem_candidates: Vec<RedeemCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConditionBucket {
    condition_id: B256,
    slug: String,
    event_slug: String,
    negative_risk: bool,
    yes_micros: u128,
    no_micros: u128,
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
        "usage: run_copytrader_account_sweeper [--root <path>] [--watch] [--interval-secs <n>] [--max-iterations <n>] [--positions-limit <n>] [--allow-live-submit] [--execution-mode <auto|direct_rpc|relayer_api|builder_api|clob_l2_hook>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--watch" => options.watch = true,
            "--interval-secs" => {
                options.interval_secs = next_value(&mut iter, arg)?
                    .parse::<u64>()
                    .map_err(|_| "invalid integer for interval-secs".to_string())?
                    .max(1)
            }
            "--max-iterations" => {
                options.max_iterations = Some(
                    next_value(&mut iter, arg)?
                        .parse::<usize>()
                        .map_err(|_| "invalid integer for max-iterations".to_string())?
                        .max(1),
                )
            }
            "--positions-limit" => {
                options.positions_limit = next_value(&mut iter, arg)?
                    .parse::<i32>()
                    .map_err(|_| "invalid integer for positions-limit".to_string())?
            }
            "--allow-live-submit" => options.allow_live_submit = true,
            "--execution-mode" => options.execution_mode = next_value(&mut iter, arg)?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if options.positions_limit < 0 || options.positions_limit > 500 {
        return Err("positions-limit must be between 0 and 500".to_string());
    }
    if parse_execution_transport(&options.execution_mode).is_none() {
        return Err(
            "execution-mode must be one of auto|direct_rpc|relayer_api|builder_api|clob_l2_hook"
                .to_string(),
        );
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
    let account_address =
        effective_funder_address(&material)?.unwrap_or_else(|| material.poly_address.clone());
    let user = SdkAddress::from_str(&account_address)
        .map_err(|error| format!("invalid effective account address {account_address}: {error}"))?;
    let client = SdkDataClient::default();
    let execution_profile = resolve_execution_profile(&root, &material, options)?;

    let mut iteration = 0usize;
    loop {
        iteration += 1;
        match sweep_iteration(
            &client,
            user,
            &root,
            &material,
            &execution_profile,
            options,
            iteration,
        )
        .await
        {
            Ok(lines) => {
                for line in lines {
                    println!("{line}");
                }
            }
            Err(error) => {
                eprintln!("[error]: account_sweeper iteration={iteration} error={error}");
                if !options.watch {
                    return Err(error);
                }
            }
        }

        if !options.watch {
            return Ok(());
        }
        if let Some(max_iterations) = options.max_iterations
            && iteration >= max_iterations
        {
            return Ok(());
        }
        sleep(Duration::from_secs(options.interval_secs)).await;
    }
}

async fn sweep_iteration(
    client: &SdkDataClient,
    user: SdkAddress,
    root: &Path,
    material: &AuthMaterial,
    execution_profile: &ExecutionProfile,
    options: &Options,
    iteration: usize,
) -> Result<Vec<String>, String> {
    let merge_positions = fetch_positions(client, user, options, PositionFilter::Mergeable).await?;
    let redeem_positions =
        fetch_positions(client, user, options, PositionFilter::Redeemable).await?;
    let plan = SweepPlan {
        merge_positions_scanned: merge_positions.len(),
        redeem_positions_scanned: redeem_positions.len(),
        merge_candidates: build_merge_candidates(&merge_positions)?,
        redeem_candidates: build_redeem_candidates(&redeem_positions)?,
    };

    let mut lines = vec![format!(
        "[info]: account_sweeper iteration={iteration} mode={} auth_env_source={} account={} execution_transport={} wallet_kind={} signature_type={} l2_auth_available={} merge_positions={} merge_candidates={} redeem_positions={} redeem_candidates={}",
        if options.allow_live_submit {
            "live"
        } else {
            "preview"
        },
        auth_env_source(root),
        effective_funder_address(material)?.unwrap_or_else(|| material.poly_address.clone()),
        execution_profile.transport.label(),
        execution_profile.wallet_kind.label(),
        execution_profile.signature_type,
        execution_profile.has_clob_l2_auth,
        plan.merge_positions_scanned,
        plan.merge_candidates.len(),
        plan.redeem_positions_scanned,
        plan.redeem_candidates.len(),
    )];

    if plan.merge_candidates.is_empty() && plan.redeem_candidates.is_empty() {
        lines.push("[info]: nothing_to_do".to_string());
        return Ok(lines);
    }

    if options.allow_live_submit && execution_profile.transport != ExecutionTransport::DirectRpc {
        lines.push(format!(
            "[warn]: execution_transport={} selected_for_signature_type={} current_live_hook=placeholder",
            execution_profile.transport.label(),
            execution_profile.signature_type,
        ));
    }

    for candidate in &plan.merge_candidates {
        if options.allow_live_submit {
            match submit_merge(root, execution_profile, candidate).await {
                Ok((tx_hash, block_number)) => lines.push(format!(
                    "[info]: merge submitted condition_id={} shares={} yes_shares={} no_shares={} negative_risk={} slug={} tx_hash={} block_number={}",
                    candidate.condition_id,
                    format_amount(candidate.merge_micros),
                    format_amount(candidate.yes_micros),
                    format_amount(candidate.no_micros),
                    candidate.negative_risk,
                    safe_slug(&candidate.slug, &candidate.event_slug),
                    tx_hash,
                    block_number,
                )),
                Err(error) => lines.push(format!(
                    "[error]: merge failed condition_id={} shares={} slug={} error={}",
                    candidate.condition_id,
                    format_amount(candidate.merge_micros),
                    safe_slug(&candidate.slug, &candidate.event_slug),
                    error,
                )),
            }
        } else {
            lines.push(format!(
                "[info]: merge preview condition_id={} shares={} yes_shares={} no_shares={} negative_risk={} slug={}",
                candidate.condition_id,
                format_amount(candidate.merge_micros),
                format_amount(candidate.yes_micros),
                format_amount(candidate.no_micros),
                candidate.negative_risk,
                safe_slug(&candidate.slug, &candidate.event_slug),
            ));
        }
    }

    for candidate in &plan.redeem_candidates {
        if options.allow_live_submit {
            match submit_redeem(root, execution_profile, candidate).await {
                Ok((tx_hash, block_number, method)) => lines.push(format!(
                    "[info]: redeem submitted condition_id={} method={} yes_shares={} no_shares={} negative_risk={} slug={} tx_hash={} block_number={}",
                    candidate.condition_id,
                    method,
                    format_amount(candidate.yes_micros),
                    format_amount(candidate.no_micros),
                    candidate.negative_risk,
                    safe_slug(&candidate.slug, &candidate.event_slug),
                    tx_hash,
                    block_number,
                )),
                Err(error) => lines.push(format!(
                    "[error]: redeem failed condition_id={} yes_shares={} no_shares={} slug={} error={}",
                    candidate.condition_id,
                    format_amount(candidate.yes_micros),
                    format_amount(candidate.no_micros),
                    safe_slug(&candidate.slug, &candidate.event_slug),
                    error,
                )),
            }
        } else {
            lines.push(format!(
                "[info]: redeem preview condition_id={} yes_shares={} no_shares={} negative_risk={} slug={}",
                candidate.condition_id,
                format_amount(candidate.yes_micros),
                format_amount(candidate.no_micros),
                candidate.negative_risk,
                safe_slug(&candidate.slug, &candidate.event_slug),
            ));
        }
    }

    Ok(lines)
}

async fn fetch_positions(
    client: &SdkDataClient,
    user: SdkAddress,
    options: &Options,
    filter: PositionFilter,
) -> Result<Vec<DataPosition>, String> {
    let page_limit = options.positions_limit.max(1);
    let mut offset = 0_i32;
    let mut positions = Vec::new();

    loop {
        let request = match filter {
            PositionFilter::Mergeable => PositionsRequest::builder()
                .user(user)
                .size_threshold(SdkDecimal::ZERO)
                .mergeable(true)
                .limit(page_limit)
                .map_err(|error| format!("invalid positions-limit: {error}"))?
                .offset(offset)
                .map_err(|error| format!("invalid positions offset {offset}: {error}"))?
                .build(),
            PositionFilter::Redeemable => PositionsRequest::builder()
                .user(user)
                .size_threshold(SdkDecimal::ZERO)
                .redeemable(true)
                .limit(page_limit)
                .map_err(|error| format!("invalid positions-limit: {error}"))?
                .offset(offset)
                .map_err(|error| format!("invalid positions offset {offset}: {error}"))?
                .build(),
        };
        let page = client
            .positions(&request)
            .await
            .map_err(|error| format!("positions endpoint failed: {error}"))?;
        let page_len = page.len() as i32;
        positions.extend(page);
        if page_len == 0 || page_len < page_limit {
            break;
        }
        offset += page_len;
        if offset > 10_000 {
            break;
        }
    }

    Ok(positions)
}

fn build_merge_candidates(positions: &[DataPosition]) -> Result<Vec<MergeCandidate>, String> {
    let mut candidates = aggregate_positions(positions)?
        .into_values()
        .filter_map(|bucket| {
            let merge_micros = min(bucket.yes_micros, bucket.no_micros);
            if merge_micros == 0 {
                return None;
            }
            Some(MergeCandidate {
                condition_id: bucket.condition_id,
                slug: bucket.slug,
                event_slug: bucket.event_slug,
                negative_risk: bucket.negative_risk,
                yes_micros: bucket.yes_micros,
                no_micros: bucket.no_micros,
                merge_micros,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then(left.condition_id.cmp(&right.condition_id))
    });
    Ok(candidates)
}

fn build_redeem_candidates(positions: &[DataPosition]) -> Result<Vec<RedeemCandidate>, String> {
    let mut candidates = aggregate_positions(positions)?
        .into_values()
        .filter_map(|bucket| {
            if bucket.yes_micros == 0 && bucket.no_micros == 0 {
                return None;
            }
            Some(RedeemCandidate {
                condition_id: bucket.condition_id,
                slug: bucket.slug,
                event_slug: bucket.event_slug,
                negative_risk: bucket.negative_risk,
                yes_micros: bucket.yes_micros,
                no_micros: bucket.no_micros,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then(left.condition_id.cmp(&right.condition_id))
    });
    Ok(candidates)
}

fn aggregate_positions(
    positions: &[DataPosition],
) -> Result<BTreeMap<String, ConditionBucket>, String> {
    let mut buckets = BTreeMap::<String, ConditionBucket>::new();
    for position in positions {
        let key = position.condition_id.to_string();
        let bucket = buckets.entry(key).or_insert_with(|| ConditionBucket {
            condition_id: position.condition_id,
            slug: position.slug.clone(),
            event_slug: position.event_slug.clone(),
            negative_risk: position.negative_risk,
            yes_micros: 0,
            no_micros: 0,
        });
        let size_micros = decimal_to_fixed_6(&position.size.to_string())?;
        if is_yes_outcome(position) {
            bucket.yes_micros = bucket.yes_micros.saturating_add(size_micros);
        } else {
            bucket.no_micros = bucket.no_micros.saturating_add(size_micros);
        }
        if bucket.slug.is_empty() {
            bucket.slug = position.slug.clone();
        }
        if bucket.event_slug.is_empty() {
            bucket.event_slug = position.event_slug.clone();
        }
        bucket.negative_risk = bucket.negative_risk || position.negative_risk;
    }
    Ok(buckets)
}

fn is_yes_outcome(position: &DataPosition) -> bool {
    position.outcome.eq_ignore_ascii_case("yes") || position.outcome_index == 0
}

async fn submit_merge(
    root: &Path,
    execution_profile: &ExecutionProfile,
    candidate: &MergeCandidate,
) -> Result<(String, String), String> {
    if execution_profile.transport != ExecutionTransport::DirectRpc {
        return Err(execution_transport_placeholder_error(execution_profile));
    }
    if execution_profile.transport != ExecutionTransport::DirectRpc {
        return Err(execution_transport_placeholder_error(execution_profile));
    }
    let rpc_urls = rpc_urls_to_try(root)?;
    let mut last_error = None;
    for rpc_url in rpc_urls {
        match submit_merge_via_rpc(root, execution_profile, candidate, &rpc_url).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                let retry_default = should_try_next_rpc(&error, &rpc_url);
                last_error = Some(error);
                if retry_default {
                    continue;
                }
                break;
            }
        }
    }
    Err(last_error
        .unwrap_or_else(|| "failed to merge positions: no rpc attempts were made".to_string()))
}

async fn submit_merge_via_rpc(
    root: &Path,
    execution_profile: &ExecutionProfile,
    candidate: &MergeCandidate,
    rpc_url: &str,
) -> Result<(String, String), String> {
    let signer = private_key_signer(root)?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect(rpc_url)
        .await
        .map_err(|error| format!("failed to connect polygon rpc {rpc_url}: {error}"))?;
    if execution_profile.wallet_kind == WalletKind::Proxy {
        return submit_merge_via_proxy_factory(provider, execution_profile, candidate, rpc_url)
            .await;
    }
    let client = CtfClient::with_neg_risk(provider, POLYGON)
        .map_err(|error| format!("failed to initialize ctf client via {rpc_url}: {error}"))?;
    let collateral_token = SdkAddress::from_str(POLYGON_PUSD)
        .map_err(|error| format!("invalid pUSD address: {error}"))?;
    let request = MergePositionsRequest::for_binary_market(
        collateral_token,
        candidate.condition_id,
        U256::from(candidate.merge_micros),
    );
    let response = client.merge_positions(&request).await.map_err(|error| {
        enrich_direct_rpc_error(
            &format!("failed to merge positions via {rpc_url}: {error}"),
            execution_profile,
        )
    })?;
    Ok((
        response.transaction_hash.to_string(),
        response.block_number.to_string(),
    ))
}

async fn submit_redeem(
    root: &Path,
    execution_profile: &ExecutionProfile,
    candidate: &RedeemCandidate,
) -> Result<(String, String, &'static str), String> {
    if execution_profile.transport != ExecutionTransport::DirectRpc {
        return Err(execution_transport_placeholder_error(execution_profile));
    }
    let rpc_urls = rpc_urls_to_try(root)?;
    let mut last_error = None;
    for rpc_url in rpc_urls {
        match submit_redeem_via_rpc(root, execution_profile, candidate, &rpc_url).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                let retry_default = should_try_next_rpc(&error, &rpc_url);
                last_error = Some(error);
                if retry_default {
                    continue;
                }
                break;
            }
        }
    }
    Err(last_error
        .unwrap_or_else(|| "failed to redeem positions: no rpc attempts were made".to_string()))
}

async fn submit_redeem_via_rpc(
    root: &Path,
    execution_profile: &ExecutionProfile,
    candidate: &RedeemCandidate,
    rpc_url: &str,
) -> Result<(String, String, &'static str), String> {
    let signer = private_key_signer(root)?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect(rpc_url)
        .await
        .map_err(|error| format!("failed to connect polygon rpc {rpc_url}: {error}"))?;
    if execution_profile.wallet_kind == WalletKind::Proxy {
        return submit_redeem_via_proxy_factory(provider, execution_profile, candidate, rpc_url)
            .await;
    }
    let client = CtfClient::with_neg_risk(provider.clone(), POLYGON)
        .map_err(|error| format!("failed to initialize ctf client via {rpc_url}: {error}"))?;
    let collateral_token = SdkAddress::from_str(POLYGON_PUSD)
        .map_err(|error| format!("invalid pUSD address: {error}"))?;

    if candidate.negative_risk {
        ensure_neg_risk_operator_approval_direct(provider, execution_profile, rpc_url).await?;
        let request = RedeemNegRiskRequest::builder()
            .condition_id(candidate.condition_id)
            .amounts(vec![
                U256::from(candidate.yes_micros),
                U256::from(candidate.no_micros),
            ])
            .build();
        let response = client.redeem_neg_risk(&request).await.map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to redeem neg-risk position via {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
        Ok((
            response.transaction_hash.to_string(),
            response.block_number.to_string(),
            "redeem_neg_risk",
        ))
    } else {
        let request =
            RedeemPositionsRequest::for_binary_market(collateral_token, candidate.condition_id);
        let response = client.redeem_positions(&request).await.map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to redeem positions via {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
        Ok((
            response.transaction_hash.to_string(),
            response.block_number.to_string(),
            "redeem_positions",
        ))
    }
}

async fn submit_merge_via_proxy_factory<P>(
    provider: P,
    execution_profile: &ExecutionProfile,
    candidate: &MergeCandidate,
    rpc_url: &str,
) -> Result<(String, String), String>
where
    P: alloy::providers::Provider + Clone,
{
    let proxy_factory = IProxyWalletFactory::new(
        SdkAddress::from_str(POLYGON_PROXY_FACTORY)
            .map_err(|error| format!("invalid proxy factory address: {error}"))?,
        provider.clone(),
    );
    let calldata = IConditionalTokensLite::mergePositionsCall {
        collateralToken: SdkAddress::from_str(POLYGON_PUSD)
            .map_err(|error| format!("invalid pUSD address: {error}"))?,
        parentCollectionId: B256::ZERO,
        conditionId: candidate.condition_id,
        partition: vec![U256::from(1_u8), U256::from(2_u8)],
        amount: U256::from(candidate.merge_micros),
    }
    .abi_encode();
    let response = proxy_factory
        .proxy(vec![ProxyCall {
            typeCode: 1,
            to: SdkAddress::from_str(POLYGON_CTF)
                .map_err(|error| format!("invalid CTF address: {error}"))?,
            value: U256::ZERO,
            data: AlloyBytes::from(calldata),
        }])
        .send()
        .await
        .map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to execute proxy merge via {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
    let tx_hash = response.tx_hash().to_string();
    let receipt = response.get_receipt().await.map_err(|error| {
        enrich_direct_rpc_error(
            &format!("failed to confirm proxy merge via {rpc_url}: {error}"),
            execution_profile,
        )
    })?;
    Ok((
        tx_hash,
        receipt.block_number.unwrap_or_default().to_string(),
    ))
}

async fn submit_redeem_via_proxy_factory<P>(
    provider: P,
    execution_profile: &ExecutionProfile,
    candidate: &RedeemCandidate,
    rpc_url: &str,
) -> Result<(String, String, &'static str), String>
where
    P: alloy::providers::Provider + Clone,
{
    let proxy_factory = IProxyWalletFactory::new(
        SdkAddress::from_str(POLYGON_PROXY_FACTORY)
            .map_err(|error| format!("invalid proxy factory address: {error}"))?,
        provider.clone(),
    );
    let (target, calldata, method) = if candidate.negative_risk {
        ensure_neg_risk_operator_approval_proxy(provider.clone(), execution_profile, rpc_url)
            .await?;
        (
            SdkAddress::from_str(POLYGON_NEG_RISK_ADAPTER)
                .map_err(|error| format!("invalid neg risk adapter address: {error}"))?,
            INegRiskAdapterLite::redeemPositionsCall {
                conditionId: candidate.condition_id,
                amounts: vec![
                    U256::from(candidate.yes_micros),
                    U256::from(candidate.no_micros),
                ],
            }
            .abi_encode(),
            "redeem_neg_risk",
        )
    } else {
        (
            SdkAddress::from_str(POLYGON_CTF)
                .map_err(|error| format!("invalid CTF address: {error}"))?,
            IConditionalTokensLite::redeemPositionsCall {
                collateralToken: SdkAddress::from_str(POLYGON_PUSD)
                    .map_err(|error| format!("invalid pUSD address: {error}"))?,
                parentCollectionId: B256::ZERO,
                conditionId: candidate.condition_id,
                indexSets: vec![U256::from(1_u8), U256::from(2_u8)],
            }
            .abi_encode(),
            "redeem_positions",
        )
    };
    let response = proxy_factory
        .proxy(vec![ProxyCall {
            typeCode: 1,
            to: target,
            value: U256::ZERO,
            data: AlloyBytes::from(calldata),
        }])
        .send()
        .await
        .map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to execute proxy redeem via {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
    let tx_hash = response.tx_hash().to_string();
    let receipt = response.get_receipt().await.map_err(|error| {
        enrich_direct_rpc_error(
            &format!("failed to confirm proxy redeem via {rpc_url}: {error}"),
            execution_profile,
        )
    })?;
    Ok((
        tx_hash,
        receipt.block_number.unwrap_or_default().to_string(),
        method,
    ))
}

async fn ensure_neg_risk_operator_approval_proxy<P>(
    provider: P,
    execution_profile: &ExecutionProfile,
    rpc_url: &str,
) -> Result<(), String>
where
    P: alloy::providers::Provider + Clone,
{
    let owner = proxy_owner_address(execution_profile)?;
    let operator = SdkAddress::from_str(POLYGON_NEG_RISK_ADAPTER)
        .map_err(|error| format!("invalid neg risk adapter address: {error}"))?;
    let ctf = IERC1155Lite::new(
        SdkAddress::from_str(POLYGON_CTF)
            .map_err(|error| format!("invalid CTF address: {error}"))?,
        provider.clone(),
    );
    let approved = ctf
        .isApprovedForAll(owner, operator)
        .call()
        .await
        .map_err(|error| format!("failed to check CTF operator approval via {rpc_url}: {error}"))?;
    if approved {
        return Ok(());
    }
    let proxy_factory = IProxyWalletFactory::new(
        SdkAddress::from_str(POLYGON_PROXY_FACTORY)
            .map_err(|error| format!("invalid proxy factory address: {error}"))?,
        provider,
    );
    let calldata = IERC1155Lite::setApprovalForAllCall {
        operator,
        approved: true,
    }
    .abi_encode();
    let response = proxy_factory
        .proxy(vec![ProxyCall {
            typeCode: 1,
            to: SdkAddress::from_str(POLYGON_CTF)
                .map_err(|error| format!("invalid CTF address: {error}"))?,
            value: U256::ZERO,
            data: AlloyBytes::from(calldata),
        }])
        .send()
        .await
        .map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to approve neg-risk operator via proxy on {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
    let tx_hash = response.tx_hash().to_string();
    let receipt = response.get_receipt().await.map_err(|error| {
        enrich_direct_rpc_error(
            &format!(
                "failed to confirm neg-risk operator approval via proxy on {rpc_url}: {error}"
            ),
            execution_profile,
        )
    })?;
    println!(
        "[info]: operator approval submitted operator={} wallet_kind={} tx_hash={} block_number={}",
        operator,
        execution_profile.wallet_kind.label(),
        tx_hash,
        receipt.block_number.unwrap_or_default(),
    );
    Ok(())
}

async fn ensure_neg_risk_operator_approval_direct<P>(
    provider: P,
    execution_profile: &ExecutionProfile,
    rpc_url: &str,
) -> Result<(), String>
where
    P: alloy::providers::Provider + Clone,
{
    let owner = direct_owner_address(execution_profile)?;
    let operator = SdkAddress::from_str(POLYGON_NEG_RISK_ADAPTER)
        .map_err(|error| format!("invalid neg risk adapter address: {error}"))?;
    let ctf = IERC1155Lite::new(
        SdkAddress::from_str(POLYGON_CTF)
            .map_err(|error| format!("invalid CTF address: {error}"))?,
        provider.clone(),
    );
    let approved = ctf
        .isApprovedForAll(owner, operator)
        .call()
        .await
        .map_err(|error| format!("failed to check CTF operator approval via {rpc_url}: {error}"))?;
    if approved {
        return Ok(());
    }
    let response = ctf
        .setApprovalForAll(operator, true)
        .send()
        .await
        .map_err(|error| {
            enrich_direct_rpc_error(
                &format!("failed to approve neg-risk operator via {rpc_url}: {error}"),
                execution_profile,
            )
        })?;
    let tx_hash = response.tx_hash().to_string();
    let receipt = response.get_receipt().await.map_err(|error| {
        enrich_direct_rpc_error(
            &format!("failed to confirm neg-risk operator approval via {rpc_url}: {error}"),
            execution_profile,
        )
    })?;
    println!(
        "[info]: operator approval submitted operator={} wallet_kind={} tx_hash={} block_number={}",
        operator,
        execution_profile.wallet_kind.label(),
        tx_hash,
        receipt.block_number.unwrap_or_default(),
    );
    Ok(())
}

fn direct_owner_address(execution_profile: &ExecutionProfile) -> Result<SdkAddress, String> {
    execution_profile
        .signer_address
        .as_deref()
        .ok_or_else(|| "missing signer address for direct operator approval".to_string())
        .and_then(|value| {
            SdkAddress::from_str(value).map_err(|error| format!("invalid signer address: {error}"))
        })
}

fn proxy_owner_address(execution_profile: &ExecutionProfile) -> Result<SdkAddress, String> {
    execution_profile
        .effective_account_address
        .as_deref()
        .ok_or_else(|| "missing proxy wallet address for operator approval".to_string())
        .and_then(|value| {
            SdkAddress::from_str(value)
                .map_err(|error| format!("invalid proxy wallet address: {error}"))
        })
}

fn enrich_direct_rpc_error(error: &str, execution_profile: &ExecutionProfile) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("insufficient funds for gas") {
        let signer = execution_profile
            .signer_address
            .as_deref()
            .unwrap_or("unknown_signer");
        let owner = execution_profile
            .effective_account_address
            .as_deref()
            .unwrap_or("unknown_account");
        return format!(
            "{error}; direct_rpc pays gas from signer {signer} while the positions sit on {owner}. Fund the signer with POL/MATIC or switch to relayer gasless execution for proxy wallets"
        );
    }
    if lower.contains("erc1155: need operator approval") {
        return format!(
            "{error}; neg-risk redeem requires CTF setApprovalForAll for operator {POLYGON_NEG_RISK_ADAPTER}. The sweeper now auto-attempts that approval in direct_rpc mode, so if this persists the approval transaction itself likely failed or the wallet type requires relayer execution"
        );
    }
    error.to_string()
}

fn rpc_urls_to_try(root: &Path) -> Result<Vec<String>, String> {
    let env_map = merged_env(root)?;
    let mut urls = Vec::<String>::new();

    if let Some(list) = env_map.get("POLYGON_RPC_URLS") {
        for value in split_rpc_list(list) {
            push_rpc_url(&mut urls, value);
        }
    }
    if let Some(value) = env_map.get("POLYGON_RPC_URL") {
        push_rpc_url(&mut urls, value);
    }
    for value in DEFAULT_PUBLIC_RPC_URLS {
        push_rpc_url(&mut urls, value);
    }
    if urls.is_empty() {
        push_rpc_url(&mut urls, DEFAULT_RPC_URL);
    }
    Ok(urls)
}

fn split_rpc_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn push_rpc_url(urls: &mut Vec<String>, value: &str) {
    let normalized = value.trim();
    if normalized.is_empty() {
        return;
    }
    if urls.iter().any(|existing| existing == normalized) {
        return;
    }
    urls.push(normalized.to_string());
}

fn should_try_next_rpc(error: &str, attempted_rpc_url: &str) -> bool {
    let error = error.to_ascii_lowercase();
    let retriable = [
        "api key disabled",
        "tenant disabled",
        "http error 401",
        "http error 403",
        "rest code: 403",
        "unauthorized",
        "forbidden",
        "invalid api key",
        "failed to connect polygon rpc",
        "connection reset",
        "connect timeout",
        "timed out",
        "429",
        "rate limit",
    ]
    .iter()
    .any(|needle| error.contains(needle));
    retriable
        && attempted_rpc_url
            != DEFAULT_PUBLIC_RPC_URLS
                .last()
                .copied()
                .unwrap_or(DEFAULT_RPC_URL)
}

impl WalletKind {
    fn label(self) -> &'static str {
        match self {
            Self::Eoa => "eoa",
            Self::Proxy => "proxy",
            Self::Safe => "safe",
        }
    }
}

impl ExecutionTransport {
    fn label(self) -> &'static str {
        match self {
            Self::DirectRpc => "direct_rpc",
            Self::RelayerApi => "relayer_api",
            Self::BuilderApi => "builder_api",
            Self::ClobL2Hook => "clob_l2_hook",
        }
    }
}

fn parse_execution_transport(value: &str) -> Option<Option<ExecutionTransport>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(None),
        "direct_rpc" => Some(Some(ExecutionTransport::DirectRpc)),
        "relayer_api" => Some(Some(ExecutionTransport::RelayerApi)),
        "builder_api" => Some(Some(ExecutionTransport::BuilderApi)),
        "clob_l2_hook" => Some(Some(ExecutionTransport::ClobL2Hook)),
        _ => None,
    }
}

fn resolve_execution_profile(
    root: &Path,
    material: &AuthMaterial,
    options: &Options,
) -> Result<ExecutionProfile, String> {
    let env_map = merged_env(root)?;
    let wallet_kind = match material.signature_type {
        0 => WalletKind::Eoa,
        1 => WalletKind::Proxy,
        2 => WalletKind::Safe,
        other => {
            return Err(format!(
                "unsupported SIGNATURE_TYPE for sweeper execution: {other}"
            ));
        }
    };
    let has_clob_l2_auth = material
        .api_secret
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && !material.api_key.trim().is_empty()
        && !material.passphrase.trim().is_empty();
    let has_relayer_api_auth =
        env_has_all(&env_map, &["RELAYER_API_KEY", "RELAYER_API_KEY_ADDRESS"]);
    let has_builder_api_auth = env_has_all(
        &env_map,
        &[
            "POLY_BUILDER_API_KEY",
            "POLY_BUILDER_SECRET",
            "POLY_BUILDER_PASSPHRASE",
        ],
    );
    let transport = match parse_execution_transport(&options.execution_mode) {
        Some(Some(explicit)) => explicit,
        Some(None) => match wallet_kind {
            WalletKind::Eoa => ExecutionTransport::DirectRpc,
            WalletKind::Proxy | WalletKind::Safe => {
                if has_relayer_api_auth {
                    ExecutionTransport::RelayerApi
                } else if has_builder_api_auth {
                    ExecutionTransport::BuilderApi
                } else if has_clob_l2_auth {
                    ExecutionTransport::ClobL2Hook
                } else {
                    ExecutionTransport::DirectRpc
                }
            }
        },
        None => {
            return Err(
                "execution-mode must be one of auto|direct_rpc|relayer_api|builder_api|clob_l2_hook"
                    .to_string(),
            );
        }
    };
    Ok(ExecutionProfile {
        wallet_kind,
        transport,
        signature_type: material.signature_type,
        has_clob_l2_auth,
        has_relayer_api_auth,
        has_builder_api_auth,
        signer_address: Some(material.poly_address.clone()),
        effective_account_address: effective_funder_address(material)?,
    })
}

fn env_has_all(env_map: &BTreeMap<String, String>, keys: &[&str]) -> bool {
    keys.iter().all(|key| {
        env_map
            .get(*key)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn execution_transport_placeholder_error(execution_profile: &ExecutionProfile) -> String {
    match execution_profile.transport {
        ExecutionTransport::RelayerApi => "proxy/safe sweeper selected relayer_api transport but relayer submission is not wired yet. Keep this seam for current proxy accounts and future relayer auth; configure RELAYER_API_KEY + RELAYER_API_KEY_ADDRESS when the relayer hook lands, or force --execution-mode direct_rpc for manual testing.".to_string(),
        ExecutionTransport::BuilderApi => "proxy/safe sweeper selected builder_api transport but builder relayer submission is not wired yet. Keep this seam for future builder gasless auth; configure POLY_BUILDER_API_KEY + POLY_BUILDER_SECRET + POLY_BUILDER_PASSPHRASE when the relayer hook lands, or force --execution-mode direct_rpc for manual testing.".to_string(),
        ExecutionTransport::ClobL2Hook => "proxy/safe sweeper detected CLOB L2 auth and selected clob_l2_hook. The hook is reserved so proxy signature_type=1 accounts and future L2-auth-backed flows can share the same execution seam; current official gasless docs still require RELAYER_API_KEY or builder keys for relayer auth, so force --execution-mode direct_rpc for manual testing until the relayer hook lands.".to_string(),
        ExecutionTransport::DirectRpc => match execution_profile.wallet_kind {
            WalletKind::Proxy => "proxy sweeper is using direct_rpc. This requires the signing EOA to fund gas and may still fail because the proxy flow is better served by the relayer path.".to_string(),
            WalletKind::Safe => "safe sweeper is using direct_rpc. This requires a safe-compatible execution path and may still fail without relayer support.".to_string(),
            WalletKind::Eoa => "direct_rpc execution failed".to_string(),
        },
    }
}

fn private_key_signer(root: &Path) -> Result<PrivateKeySigner, String> {
    required_merged_env(root, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"])?
        .parse::<PrivateKeySigner>()
        .map_err(|error| format!("invalid PRIVATE_KEY: {error}"))
        .map(|signer| signer.with_chain_id(Some(POLYGON)))
}

fn decimal_to_fixed_6(value: &str) -> Result<u128, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("empty decimal value".to_string());
    }
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
    combined
        .parse::<u128>()
        .map_err(|error| format!("invalid scaled decimal {value}: {error}"))
}

fn format_amount(micros: u128) -> String {
    let whole = micros / 1_000_000;
    let frac = micros % 1_000_000;
    if frac == 0 {
        return whole.to_string();
    }
    let mut frac_text = format!("{frac:06}");
    while frac_text.ends_with('0') {
        frac_text.pop();
    }
    format!("{whole}.{frac_text}")
}

fn safe_slug(slug: &str, event_slug: &str) -> String {
    if !slug.trim().is_empty() {
        slug.to_string()
    } else if !event_slug.trim().is_empty() {
        event_slug.to_string()
    } else {
        "unknown".to_string()
    }
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
        1 => polymarket_client_sdk::derive_proxy_wallet(signer, POLYGON),
        2 => polymarket_client_sdk::derive_safe_wallet(signer, POLYGON),
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

fn required_merged_env(root: &Path, keys: &[&str]) -> Result<String, String> {
    let env_map = merged_env(root)?;
    keys.iter()
        .find_map(|key| env_map.get(*key).cloned())
        .ok_or_else(|| format!("missing field {}", keys[0]))
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
        DEFAULT_PUBLIC_RPC_URLS, DEFAULT_RPC_URL, ExecutionTransport, MergeCandidate, Options,
        RedeemCandidate, WalletKind, build_merge_candidates, build_redeem_candidates,
        decimal_to_fixed_6, format_amount, parse_args, push_rpc_url, resolve_execution_profile,
        rpc_urls_to_try, should_try_next_rpc, split_rpc_list,
    };
    use polymarket_client_sdk::data::types::response::Position as DataPosition;
    use polymarket_client_sdk::types::{Address as SdkAddress, B256, Decimal as SdkDecimal, U256};
    use std::env;
    use std::fs;
    use std::str::FromStr as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_position(condition_id: &str, outcome: &str, size: &str) -> DataPosition {
        DataPosition::builder()
            .proxy_wallet(
                SdkAddress::from_str("0x0bdc847347571342e1563971e8ba206c8b03e345").unwrap(),
            )
            .asset(U256::from(if outcome.eq_ignore_ascii_case("yes") {
                1_u64
            } else {
                2_u64
            }))
            .condition_id(B256::from_str(condition_id).unwrap())
            .size(SdkDecimal::from_str(size).unwrap())
            .avg_price(SdkDecimal::from_str("0.42").unwrap())
            .initial_value(SdkDecimal::from_str("1").unwrap())
            .current_value(SdkDecimal::from_str("1").unwrap())
            .cash_pnl(SdkDecimal::ZERO)
            .percent_pnl(SdkDecimal::ZERO)
            .total_bought(SdkDecimal::from_str("1").unwrap())
            .realized_pnl(SdkDecimal::ZERO)
            .percent_realized_pnl(SdkDecimal::ZERO)
            .cur_price(SdkDecimal::from_str("0.5").unwrap())
            .redeemable(true)
            .mergeable(true)
            .title("market title".to_string())
            .slug("market-slug".to_string())
            .icon(String::new())
            .event_slug("event-slug".to_string())
            .event_id("event-id".to_string())
            .outcome(outcome.to_string())
            .outcome_index(if outcome.eq_ignore_ascii_case("yes") {
                0
            } else {
                1
            })
            .opposite_outcome(if outcome.eq_ignore_ascii_case("yes") {
                "No".to_string()
            } else {
                "Yes".to_string()
            })
            .opposite_asset(U256::from(if outcome.eq_ignore_ascii_case("yes") {
                2_u64
            } else {
                1_u64
            }))
            .negative_risk(false)
            .build()
    }

    #[test]
    fn parse_args_accepts_watch_and_live_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--watch".into(),
            "--interval-secs".into(),
            "12".into(),
            "--max-iterations".into(),
            "3".into(),
            "--positions-limit".into(),
            "200".into(),
            "--allow-live-submit".into(),
        ])
        .expect("parse");

        assert_eq!(
            options,
            Options {
                root: "..".to_string(),
                watch: true,
                interval_secs: 12,
                max_iterations: Some(3),
                positions_limit: 200,
                allow_live_submit: true,
                execution_mode: "auto".to_string(),
            }
        );
    }

    #[test]
    fn decimal_to_fixed_6_truncates_extra_precision() {
        assert_eq!(decimal_to_fixed_6("5").expect("scaled"), 5_000_000);
        assert_eq!(decimal_to_fixed_6("0.1234567").expect("scaled"), 123_456);
        assert_eq!(decimal_to_fixed_6("12.000001").expect("scaled"), 12_000_001);
    }

    #[test]
    fn format_amount_trims_trailing_zeros() {
        assert_eq!(format_amount(5_000_000), "5");
        assert_eq!(format_amount(123_400), "0.1234");
        assert_eq!(format_amount(12_000_001), "12.000001");
    }

    #[test]
    fn build_merge_candidates_uses_min_full_set_size() {
        let condition_id = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let positions = vec![
            sample_position(condition_id, "Yes", "3.50"),
            sample_position(condition_id, "No", "1.25"),
        ];

        let candidates = build_merge_candidates(&positions).expect("candidates");
        assert_eq!(
            candidates,
            vec![MergeCandidate {
                condition_id: B256::from_str(condition_id).unwrap(),
                slug: "market-slug".to_string(),
                event_slug: "event-slug".to_string(),
                negative_risk: false,
                yes_micros: 3_500_000,
                no_micros: 1_250_000,
                merge_micros: 1_250_000,
            }]
        );
    }

    #[test]
    fn rpc_urls_to_try_falls_back_to_default_when_custom_url_is_present() {
        let root = std::env::temp_dir();
        unsafe {
            env::set_var("POLYGON_RPC_URL", "https://rpc.example.invalid/key");
        }
        let urls = rpc_urls_to_try(&root).expect("urls");
        assert_eq!(urls[0], "https://rpc.example.invalid/key");
        assert!(urls.iter().any(|url| url == DEFAULT_RPC_URL));
        assert!(urls.iter().any(|url| url == DEFAULT_PUBLIC_RPC_URLS[1]));
        unsafe {
            env::remove_var("POLYGON_RPC_URL");
        }
    }

    #[test]
    fn should_try_next_rpc_matches_disabled_rpc_tenant_errors() {
        assert!(should_try_next_rpc(
            r#"failed to redeem positions via https://rpc.example: HTTP error 401 with body: {"error":"message: API key disabled, reason: tenant disabled, json-rpc code: -32051, rest code: 403"}"#,
            "https://rpc.example"
        ));
        assert!(should_try_next_rpc(
            "failed to connect polygon rpc https://rpc.example: connection reset by peer",
            "https://rpc.example"
        ));
        assert!(!should_try_next_rpc(
            "failed to redeem positions via https://polygon.drpc.org: execution reverted",
            DEFAULT_RPC_URL
        ));
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-copytrader-account-sweeper-{name}-{suffix}"))
    }

    #[test]
    fn resolve_execution_profile_prefers_clob_l2_hook_for_proxy_accounts() {
        let root = unique_temp_dir("proxy-l2");
        fs::create_dir_all(&root).expect("dir created");
        fs::write(
            root.join(".env"),
            "PRIVATE_KEY=0x59c6995e998f97a5a0044966f094538c5f34f6c4a0499b6f6f489f5fabe59d3f
CLOB_API_KEY=00000000-0000-0000-0000-000000000001
CLOB_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
CLOB_PASS_PHRASE=test-pass
SIGNATURE_TYPE=1
FUNDER_ADDRESS=0x0bDC847347571342E1563971E8bA206c8B03e345
",
        )
        .expect("env written");
        let material = super::auth_material_with_signer_fallback(&root).expect("material");
        let profile =
            resolve_execution_profile(&root, &material, &Options::default()).expect("profile");
        assert_eq!(profile.wallet_kind, WalletKind::Proxy);
        assert_eq!(profile.transport, ExecutionTransport::ClobL2Hook);
        assert!(profile.has_clob_l2_auth);
    }

    #[test]
    fn resolve_execution_profile_honors_explicit_direct_override() {
        let root = unique_temp_dir("proxy-direct");
        fs::create_dir_all(&root).expect("dir created");
        fs::write(
            root.join(".env"),
            "PRIVATE_KEY=0x59c6995e998f97a5a0044966f094538c5f34f6c4a0499b6f6f489f5fabe59d3f
CLOB_API_KEY=00000000-0000-0000-0000-000000000001
CLOB_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
CLOB_PASS_PHRASE=test-pass
SIGNATURE_TYPE=1
FUNDER_ADDRESS=0x0bDC847347571342E1563971E8bA206c8B03e345
",
        )
        .expect("env written");
        let material = super::auth_material_with_signer_fallback(&root).expect("material");
        let profile = resolve_execution_profile(
            &root,
            &material,
            &Options {
                execution_mode: "direct_rpc".to_string(),
                ..Options::default()
            },
        )
        .expect("profile");
        assert_eq!(profile.transport, ExecutionTransport::DirectRpc);
    }

    #[test]
    fn split_rpc_list_accepts_commas_semicolons_and_spaces() {
        let values = split_rpc_list(
            "https://a.example, https://b.example;https://c.example  https://d.example",
        )
        .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                "https://a.example",
                "https://b.example",
                "https://c.example",
                "https://d.example"
            ]
        );
    }

    #[test]
    fn push_rpc_url_deduplicates_entries() {
        let mut urls = Vec::new();
        push_rpc_url(&mut urls, "https://polygon.drpc.org");
        push_rpc_url(&mut urls, "https://polygon.drpc.org");
        push_rpc_url(&mut urls, "  https://polygon.publicnode.com  ");
        assert_eq!(
            urls,
            vec![
                "https://polygon.drpc.org".to_string(),
                "https://polygon.publicnode.com".to_string(),
            ]
        );
    }
    #[test]
    fn build_redeem_candidates_groups_by_condition() {
        let one = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let two = "0x2222222222222222222222222222222222222222222222222222222222222222";
        let positions = vec![
            sample_position(one, "Yes", "4.0"),
            sample_position(one, "No", "0.5"),
            sample_position(two, "Yes", "1.0"),
        ];

        let candidates = build_redeem_candidates(&positions).expect("candidates");
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0],
            RedeemCandidate {
                condition_id: B256::from_str(one).unwrap(),
                slug: "market-slug".to_string(),
                event_slug: "event-slug".to_string(),
                negative_risk: false,
                yes_micros: 4_000_000,
                no_micros: 500_000,
            }
        );
        assert_eq!(candidates[1].condition_id, B256::from_str(two).unwrap());
        assert_eq!(candidates[1].yes_micros, 1_000_000);
        assert_eq!(candidates[1].no_micros, 0);
    }
}
