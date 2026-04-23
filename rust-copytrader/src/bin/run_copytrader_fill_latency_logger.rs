use futures::StreamExt as _;
use polymarket_client_sdk::auth::{Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid};
use polymarket_client_sdk::clob::types::TraderSide;
use polymarket_client_sdk::clob::ws::types::response::{MakerOrder, OrderMessage, TradeMessage};
use polymarket_client_sdk::clob::ws::{Client as WsClient, WsMessage};
use polymarket_client_sdk::types::Address as SdkAddress;
use polymarket_client_sdk::{POLYGON, derive_proxy_wallet, derive_safe_wallet};
use rust_copytrader::adapters::signing::AuthMaterial;
use rust_copytrader::config::{RootEnvLoadError, is_valid_evm_wallet};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr as _;
use tokio::time::{Duration, MissedTickBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    user: String,
    summary_root: Option<String>,
    log_dir: Option<String>,
    poll_interval_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            user: String::new(),
            summary_root: None,
            log_dir: None,
            poll_interval_ms: 250,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestActivityInfo {
    asset_id: String,
    slug: Option<String>,
    outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryContext {
    summary_path: PathBuf,
    run_dir: PathBuf,
    leader_wallet: String,
    leader_tx: String,
    leader_timestamp_ms: i64,
    leader_price: Option<String>,
    follow_share_size: Option<String>,
    order_id: Option<String>,
    trade_ids: BTreeSet<String>,
    transaction_hashes: BTreeSet<String>,
    latest_activity: LatestActivityInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingFill {
    summary: SummaryContext,
    logged_trade_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TradeSnapshot {
    id: String,
    fill_timestamp_ms: i64,
    fill_timestamp_source: &'static str,
    order_ids: BTreeSet<String>,
    transaction_hash: Option<String>,
    price: String,
    size: String,
    asset_id: String,
    market_id: String,
    outcome: Option<String>,
    trader_side: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrderSnapshot {
    id: String,
    associated_trades: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LatencyRecord {
    leader_wallet: String,
    leader_tx: String,
    leader_timestamp_ms: i64,
    fill_timestamp_ms: i64,
    latency_ms: i64,
    leader_price: Option<f64>,
    fill_price: f64,
    price_gap: Option<f64>,
    price_gap_bps: Option<f64>,
    shares: f64,
    requested_follow_shares: Option<f64>,
    order_id: Option<String>,
    trade_id: String,
    transaction_hash: Option<String>,
    asset_id: String,
    market_id: String,
    slug: Option<String>,
    outcome: Option<String>,
    fill_timestamp_source: &'static str,
    summary_path: PathBuf,
    run_dir: PathBuf,
    trader_side: Option<String>,
}

#[derive(Debug, Default)]
struct Tracker {
    seen_summary_paths: BTreeSet<String>,
    pending: HashMap<String, PendingFill>,
    order_index: HashMap<String, BTreeSet<String>>,
    trade_index: HashMap<String, BTreeSet<String>>,
    tx_index: HashMap<String, BTreeSet<String>>,
    order_cache: HashMap<String, OrderSnapshot>,
    trade_cache: HashMap<String, TradeSnapshot>,
}

impl Tracker {
    fn seed_existing(&mut self, existing_summary_paths: impl IntoIterator<Item = String>) {
        self.seen_summary_paths.extend(existing_summary_paths);
    }

    fn ingest_summary(&mut self, summary: SummaryContext) -> Result<Vec<LatencyRecord>, String> {
        let key = summary.summary_path.display().to_string();
        if !self.seen_summary_paths.insert(key.clone()) {
            return Ok(Vec::new());
        }

        let trade_ids = summary.trade_ids.clone();
        let transaction_hashes = summary.transaction_hashes.clone();
        let order_id = summary.order_id.clone();
        self.pending.insert(
            key.clone(),
            PendingFill {
                summary,
                logged_trade_ids: BTreeSet::new(),
            },
        );

        if let Some(order_id) = order_id.as_deref() {
            self.order_index
                .entry(order_id.to_string())
                .or_default()
                .insert(key.clone());
            if let Some(order) = self.order_cache.get(order_id) {
                for trade_id in &order.associated_trades {
                    self.trade_index
                        .entry(trade_id.clone())
                        .or_default()
                        .insert(key.clone());
                    if let Some(pending) = self.pending.get_mut(&key) {
                        pending.summary.trade_ids.insert(trade_id.clone());
                    }
                }
            }
        }

        for trade_id in trade_ids {
            self.trade_index
                .entry(trade_id)
                .or_default()
                .insert(key.clone());
        }
        for tx_hash in transaction_hashes {
            self.tx_index
                .entry(tx_hash)
                .or_default()
                .insert(key.clone());
        }

        self.collect_matches_for_summary(&key)
    }

    fn ingest_order(&mut self, order: OrderSnapshot) -> Result<Vec<LatencyRecord>, String> {
        let mut records = Vec::new();
        let order_id = order.id.clone();
        self.order_cache.insert(order.id.clone(), order.clone());
        if let Some(summary_keys) = self.order_index.get(&order_id).cloned() {
            for summary_key in summary_keys {
                for trade_id in &order.associated_trades {
                    self.trade_index
                        .entry(trade_id.clone())
                        .or_default()
                        .insert(summary_key.clone());
                    if let Some(pending) = self.pending.get_mut(&summary_key) {
                        pending.summary.trade_ids.insert(trade_id.clone());
                    }
                }
                records.extend(self.collect_matches_for_summary(&summary_key)?);
            }
        }
        Ok(records)
    }

    fn ingest_trade(&mut self, trade: TradeSnapshot) -> Result<Vec<LatencyRecord>, String> {
        let trade_id = trade.id.clone();
        let order_ids = trade.order_ids.clone();
        let tx_hash = trade.transaction_hash.clone();
        self.trade_cache.insert(trade_id.clone(), trade);

        let mut summary_keys = BTreeSet::new();
        if let Some(keys) = self.trade_index.get(&trade_id) {
            summary_keys.extend(keys.iter().cloned());
        }
        for order_id in order_ids {
            if let Some(keys) = self.order_index.get(&order_id) {
                summary_keys.extend(keys.iter().cloned());
            }
        }
        if let Some(tx_hash) = tx_hash.as_deref()
            && let Some(keys) = self.tx_index.get(tx_hash)
        {
            summary_keys.extend(keys.iter().cloned());
        }

        let mut records = Vec::new();
        for summary_key in summary_keys {
            records.extend(self.collect_matches_for_summary(&summary_key)?);
        }
        Ok(records)
    }

    fn collect_matches_for_summary(
        &mut self,
        summary_key: &str,
    ) -> Result<Vec<LatencyRecord>, String> {
        let Some(pending) = self.pending.get(summary_key) else {
            return Ok(Vec::new());
        };

        let mut candidate_ids = pending.summary.trade_ids.clone();
        if let Some(order_id) = pending.summary.order_id.as_deref() {
            for trade in self.trade_cache.values() {
                if trade.order_ids.contains(order_id) {
                    candidate_ids.insert(trade.id.clone());
                }
            }
        }
        for tx_hash in &pending.summary.transaction_hashes {
            for trade in self.trade_cache.values() {
                if trade.transaction_hash.as_deref() == Some(tx_hash.as_str()) {
                    candidate_ids.insert(trade.id.clone());
                }
            }
        }

        let mut records = Vec::new();
        for trade_id in candidate_ids {
            let Some(trade) = self.trade_cache.get(&trade_id).cloned() else {
                continue;
            };
            if let Some(record) = self.try_build_record(summary_key, &trade)? {
                records.push(record);
            }
        }
        Ok(records)
    }

    fn try_build_record(
        &mut self,
        summary_key: &str,
        trade: &TradeSnapshot,
    ) -> Result<Option<LatencyRecord>, String> {
        let Some(pending) = self.pending.get_mut(summary_key) else {
            return Ok(None);
        };
        if pending.logged_trade_ids.contains(&trade.id) {
            return Ok(None);
        }

        if pending.summary.latest_activity.asset_id != trade.asset_id {
            return Ok(None);
        }

        let leader_price = pending
            .summary
            .leader_price
            .as_deref()
            .map(parse_f64)
            .transpose()?;
        let fill_price = parse_f64(&trade.price)?;
        let shares = parse_f64(&trade.size)?;
        let requested_follow_shares = pending
            .summary
            .follow_share_size
            .as_deref()
            .map(parse_f64)
            .transpose()?;
        let latency_ms = trade.fill_timestamp_ms - pending.summary.leader_timestamp_ms;
        let price_gap = leader_price.map(|leader_price| fill_price - leader_price);
        let price_gap_bps = match leader_price {
            Some(leader_price) if leader_price > 0.0 => {
                Some(((fill_price - leader_price) / leader_price) * 10_000.0)
            }
            _ => None,
        };

        pending.logged_trade_ids.insert(trade.id.clone());

        Ok(Some(LatencyRecord {
            leader_wallet: pending.summary.leader_wallet.clone(),
            leader_tx: pending.summary.leader_tx.clone(),
            leader_timestamp_ms: pending.summary.leader_timestamp_ms,
            fill_timestamp_ms: trade.fill_timestamp_ms,
            latency_ms,
            leader_price,
            fill_price,
            price_gap,
            price_gap_bps,
            shares,
            requested_follow_shares,
            order_id: pending.summary.order_id.clone(),
            trade_id: trade.id.clone(),
            transaction_hash: trade.transaction_hash.clone(),
            asset_id: trade.asset_id.clone(),
            market_id: trade.market_id.clone(),
            slug: pending.summary.latest_activity.slug.clone(),
            outcome: pending
                .summary
                .latest_activity
                .outcome
                .clone()
                .or_else(|| trade.outcome.clone()),
            fill_timestamp_source: trade.fill_timestamp_source,
            summary_path: pending.summary.summary_path.clone(),
            run_dir: pending.summary.run_dir.clone(),
            trader_side: trade.trader_side.clone(),
        }))
    }
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

    match tokio_main(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", enrich_run_error(&error));
            ExitCode::from(1)
        }
    }
}

fn tokio_main(options: Options) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build tokio runtime: {error}"))?;
    runtime.block_on(run(options))
}

fn enrich_run_error(error: &str) -> String {
    let mut message = error.to_string();
    if error.contains("missing private key") || error.contains("missing field PRIVATE_KEY") {
        message.push_str("
fill latency logger needs follower auth to subscribe to the authenticated user websocket. Set PRIVATE_KEY or CLOB_PRIVATE_KEY plus CLOB_API_KEY, CLOB_SECRET, and CLOB_PASS_PHRASE in the repo .env/.env.local or process env.");
    } else if error.contains("missing CLOB_SECRET") {
        message.push_str("
fill latency logger uses the follower account websocket and requires CLOB_API_KEY, CLOB_SECRET, CLOB_PASS_PHRASE, and the private key in local env.");
    }
    message
}

fn print_usage() {
    println!(
        "usage: run_copytrader_fill_latency_logger --user <leader-wallet> [--root <path>] [--summary-root <path>] [--log-dir <path>] [--poll-interval-ms <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--user" => options.user = next_value(&mut iter, arg)?,
            "--summary-root" => options.summary_root = Some(next_value(&mut iter, arg)?),
            "--log-dir" => options.log_dir = Some(next_value(&mut iter, arg)?),
            "--poll-interval-ms" => {
                options.poll_interval_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "poll-interval-ms")?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if options.user.trim().is_empty() {
        return Err("missing --user <leader-wallet>".to_string());
    }
    if !is_valid_evm_wallet(&options.user) {
        return Err(format!("invalid leader wallet: {}", options.user));
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

async fn run(options: Options) -> Result<(), String> {
    let root = PathBuf::from(&options.root);
    let leader_key = sanitize_for_filename(&options.user);
    let summary_root = options
        .summary_root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(".omx")
                .join("force-live-follow")
                .join(&leader_key)
                .join("runs")
        });
    let log_dir = options
        .log_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx").join("fill-latency").join(&leader_key));
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("failed to create {}: {error}", log_dir.display()))?;
    let fills_log_path = log_dir.join("fills.log");
    let fills_jsonl_path = log_dir.join("fills.jsonl");

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

    let mut tracker = Tracker::default();
    tracker.seed_existing(existing_summary_paths(&summary_root)?);

    println!(
        "fill_latency_logger_ready user={} fills_log={} fills_jsonl={} summary_root={}",
        options.user,
        fills_log_path.display(),
        fills_jsonl_path.display(),
        summary_root.display()
    );

    let mut interval =
        tokio::time::interval(Duration::from_millis(options.poll_interval_ms.max(50)));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let summaries = scan_new_summaries(&summary_root)?;
                for summary in summaries {
                    let records = tracker.ingest_summary(summary)?;
                    for record in records {
                        emit_record(&record, &fills_log_path, &fills_jsonl_path)?;
                    }
                }
            }
            maybe_event = stream.next() => {
                let event = maybe_event.ok_or_else(|| "user websocket stream ended".to_string())?
                    .map_err(|error| format!("user websocket stream error: {error}"))?;
                match event {
                    WsMessage::Trade(trade) => {
                        let records = tracker.ingest_trade(trade_snapshot_from_ws(&trade)?)?;
                        for record in records {
                            emit_record(&record, &fills_log_path, &fills_jsonl_path)?;
                        }
                    }
                    WsMessage::Order(order) => {
                        let records = tracker.ingest_order(order_snapshot_from_ws(&order))?;
                        for record in records {
                            emit_record(&record, &fills_log_path, &fills_jsonl_path)?;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn existing_summary_paths(summary_root: &Path) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    if !summary_root.exists() {
        return Ok(paths);
    }
    for entry in fs::read_dir(summary_root)
        .map_err(|error| format!("failed to read {}: {error}", summary_root.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read summary dir entry: {error}"))?;
        let summary_path = entry.path().join("summary.txt");
        if summary_path.exists() {
            paths.push(summary_path.display().to_string());
        }
    }
    Ok(paths)
}

fn scan_new_summaries(summary_root: &Path) -> Result<Vec<SummaryContext>, String> {
    let mut summaries = Vec::new();
    if !summary_root.exists() {
        return Ok(summaries);
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(summary_root)
        .map_err(|error| format!("failed to read {}: {error}", summary_root.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read summary dir entry: {error}"))?;
        let summary_path = entry.path().join("summary.txt");
        if summary_path.exists() {
            paths.push(summary_path);
        }
    }
    paths.sort();
    for summary_path in paths {
        if let Some(summary) = read_summary_context(&summary_path)? {
            summaries.push(summary);
        }
    }
    Ok(summaries)
}

fn read_summary_context(path: &Path) -> Result<Option<SummaryContext>, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let values = body
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect::<BTreeMap<_, _>>();

    if values.get("status").map(String::as_str) != Some("submit_completed") {
        return Ok(None);
    }
    if values
        .get("latest_activity_type")
        .map(String::as_str)
        .unwrap_or_default()
        != "TRADE"
    {
        return Ok(None);
    }

    let summary_dir = path
        .parent()
        .ok_or_else(|| format!("summary path has no parent: {}", path.display()))?;
    let selected_activity_path = values
        .get("selected_latest_activity")
        .or_else(|| values.get("selected_latest_activity_path"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing selected_latest_activity in {}", path.display()))?;
    let latest_activity = read_latest_activity_info(&selected_activity_path)?;

    let leader_timestamp_secs = values
        .get("latest_activity_timestamp")
        .or_else(|| values.get("latest_timestamp"))
        .ok_or_else(|| format!("missing latest activity timestamp in {}", path.display()))?
        .parse::<i64>()
        .map_err(|error| {
            format!(
                "invalid latest activity timestamp in {}: {error}",
                path.display()
            )
        })?;
    let trade_ids = split_csv(values.get("submit_trade_ids").map(String::as_str));
    let transaction_hashes = split_csv(values.get("submit_transaction_hashes").map(String::as_str));

    Ok(Some(SummaryContext {
        summary_path: path.to_path_buf(),
        run_dir: summary_dir.to_path_buf(),
        leader_wallet: values.get("user").cloned().unwrap_or_default(),
        leader_tx: values.get("latest_tx").cloned().unwrap_or_default(),
        leader_timestamp_ms: leader_timestamp_secs.saturating_mul(1000),
        leader_price: values
            .get("latest_activity_price")
            .cloned()
            .or_else(|| values.get("leader_price").cloned()),
        follow_share_size: values
            .get("follow_share_size")
            .cloned()
            .or_else(|| values.get("order_size").cloned()),
        order_id: values
            .get("submit_order_id")
            .cloned()
            .filter(|value| !value.is_empty()),
        trade_ids,
        transaction_hashes,
        latest_activity,
    }))
}

fn split_csv(value: Option<&str>) -> BTreeSet<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn read_latest_activity_info(path: &Path) -> Result<LatestActivityInfo, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        format!(
            "failed to parse selected latest activity {}: {error}",
            path.display()
        )
    })?;
    let object = value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "selected latest activity is not an array object: {}",
                path.display()
            )
        })?;
    let asset_id = object
        .get("asset")
        .and_then(json_string_or_number)
        .ok_or_else(|| format!("missing asset in {}", path.display()))?;
    let slug = object.get("slug").and_then(json_string_or_number);
    let outcome = object.get("outcome").and_then(json_string_or_number);
    Ok(LatestActivityInfo {
        asset_id,
        slug,
        outcome,
    })
}

fn json_string_or_number(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_f64().map(|value| value.to_string()))
}

fn trade_snapshot_from_ws(trade: &TradeMessage) -> Result<TradeSnapshot, String> {
    let (fill_timestamp_ms, fill_timestamp_source) = if let Some(value) = trade.matchtime {
        (value, "matchtime")
    } else if let Some(value) = trade.timestamp {
        (value, "timestamp")
    } else if let Some(value) = trade.last_update {
        (value, "last_update")
    } else {
        return Err(format!("trade {} missing fill timestamp", trade.id));
    };

    Ok(TradeSnapshot {
        id: trade.id.clone(),
        fill_timestamp_ms,
        fill_timestamp_source,
        order_ids: related_order_ids(trade),
        transaction_hash: trade.transaction_hash.as_ref().map(ToString::to_string),
        price: trade.price.to_string(),
        size: trade.size.to_string(),
        asset_id: trade.asset_id.to_string(),
        market_id: trade.market.to_string(),
        outcome: trade.outcome.clone(),
        trader_side: trade.trader_side.as_ref().map(render_trader_side),
    })
}

fn order_snapshot_from_ws(order: &OrderMessage) -> OrderSnapshot {
    OrderSnapshot {
        id: order.id.clone(),
        associated_trades: order
            .associate_trades
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    }
}

fn related_order_ids(trade: &TradeMessage) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    if let Some(taker_order_id) = trade.taker_order_id.as_deref()
        && !taker_order_id.trim().is_empty()
    {
        ids.insert(taker_order_id.to_string());
    }
    for MakerOrder { order_id, .. } in &trade.maker_orders {
        if !order_id.trim().is_empty() {
            ids.insert(order_id.clone());
        }
    }
    ids
}

fn render_trader_side(side: &TraderSide) -> String {
    format!("{side:?}")
}

fn emit_record(
    record: &LatencyRecord,
    fills_log_path: &Path,
    fills_jsonl_path: &Path,
) -> Result<(), String> {
    let line = render_human_line(record);
    println!("{line}");
    append_line(fills_log_path, &line)?;
    append_line(fills_jsonl_path, &render_json_line(record))?;
    Ok(())
}

fn render_human_line(record: &LatencyRecord) -> String {
    let mut parts = vec![
        "fill".to_string(),
        format!("latency_ms={}", record.latency_ms),
        format!("leader_ts_ms={}", record.leader_timestamp_ms),
        format!("fill_ts_ms={}", record.fill_timestamp_ms),
        format!(
            "leader_price={}",
            render_optional_price(record.leader_price)
        ),
        format!("fill_price={:.8}", record.fill_price),
        format!(
            "price_gap_bps={}",
            render_optional_bps(record.price_gap_bps)
        ),
        format!("shares={:.2}", record.shares),
        format!(
            "requested_shares={}",
            record
                .requested_follow_shares
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default()
        ),
        format!("leader_tx={}", record.leader_tx),
        format!("trade_id={}", record.trade_id),
    ];
    if let Some(order_id) = record.order_id.as_deref() {
        parts.push(format!("order_id={order_id}"));
    }
    if let Some(slug) = record.slug.as_deref() {
        parts.push(format!("slug={slug}"));
    }
    if let Some(outcome) = record.outcome.as_deref() {
        parts.push(format!("outcome={outcome}"));
    }
    parts.join(" ")
}

fn render_optional_price(value: Option<f64>) -> String {
    value.map(|value| format!("{value:.8}")).unwrap_or_default()
}

fn render_optional_bps(value: Option<f64>) -> String {
    value.map(|value| format!("{value:.4}")).unwrap_or_default()
}

fn render_json_line(record: &LatencyRecord) -> String {
    json!({
        "kind": "fill",
        "leader_wallet": record.leader_wallet,
        "leader_tx": record.leader_tx,
        "leader_timestamp_ms": record.leader_timestamp_ms,
        "fill_timestamp_ms": record.fill_timestamp_ms,
        "fill_timestamp_source": record.fill_timestamp_source,
        "latency_ms": record.latency_ms,
        "leader_price": record.leader_price,
        "fill_price": record.fill_price,
        "price_gap": record.price_gap,
        "price_gap_bps": record.price_gap_bps,
        "shares": record.shares,
        "requested_follow_shares": record.requested_follow_shares,
        "order_id": record.order_id,
        "trade_id": record.trade_id,
        "transaction_hash": record.transaction_hash,
        "asset_id": record.asset_id,
        "market_id": record.market_id,
        "slug": record.slug,
        "outcome": record.outcome,
        "summary_path": record.summary_path.display().to_string(),
        "run_dir": record.run_dir.display().to_string(),
        "trader_side": record.trader_side,
    })
    .to_string()
}

fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    writeln!(file, "{line}").map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn parse_f64(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid decimal value: {value}"))
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
        LatestActivityInfo, OrderSnapshot, SummaryContext, Tracker, TradeSnapshot,
        enrich_run_error, render_human_line,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn sample_summary() -> SummaryContext {
        SummaryContext {
            summary_path: PathBuf::from("/tmp/run-1/summary.txt"),
            run_dir: PathBuf::from("/tmp/run-1"),
            leader_wallet: "0xleader".into(),
            leader_tx: "0xleader-tx".into(),
            leader_timestamp_ms: 1_700_000_000_000,
            leader_price: Some("0.50000000".into()),
            follow_share_size: Some("6.000000".into()),
            order_id: Some("order-1".into()),
            trade_ids: BTreeSet::new(),
            transaction_hashes: BTreeSet::new(),
            latest_activity: LatestActivityInfo {
                asset_id: "asset-1".into(),
                slug: Some("market-a".into()),
                outcome: Some("Yes".into()),
            },
        }
    }

    fn sample_trade() -> TradeSnapshot {
        let mut order_ids = BTreeSet::new();
        order_ids.insert("order-1".into());
        TradeSnapshot {
            id: "trade-1".into(),
            fill_timestamp_ms: 1_700_000_001_250,
            fill_timestamp_source: "matchtime",
            order_ids,
            transaction_hash: Some("0xtx".into()),
            price: "0.51000000".into(),
            size: "6.00".into(),
            asset_id: "asset-1".into(),
            market_id: "market-1".into(),
            outcome: Some("Yes".into()),
            trader_side: Some("Taker".into()),
        }
    }

    #[test]
    fn tracker_matches_trade_by_order_id() {
        let mut tracker = Tracker::default();
        tracker
            .ingest_summary(sample_summary())
            .expect("summary ingested");
        let records = tracker
            .ingest_trade(sample_trade())
            .expect("trade ingested");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].latency_ms, 1_250);
        assert!((records[0].price_gap_bps.unwrap() - 200.0).abs() < 0.0001);
        assert_eq!(records[0].shares, 6.0);
    }

    #[test]
    fn tracker_matches_trade_arriving_before_summary() {
        let mut tracker = Tracker::default();
        tracker.ingest_trade(sample_trade()).expect("trade cached");
        let records = tracker
            .ingest_summary(sample_summary())
            .expect("summary ingested");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trade_id, "trade-1");
    }

    #[test]
    fn tracker_matches_order_associated_trade_then_trade() {
        let mut tracker = Tracker::default();
        tracker
            .ingest_summary(sample_summary())
            .expect("summary ingested");
        let mut associated = BTreeSet::new();
        associated.insert("trade-1".into());
        let order_records = tracker
            .ingest_order(OrderSnapshot {
                id: "order-1".into(),
                associated_trades: associated,
            })
            .expect("order ingested");
        assert!(order_records.is_empty());
        let records = tracker
            .ingest_trade(sample_trade())
            .expect("trade ingested");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trade_id, "trade-1");
    }

    #[test]
    fn render_human_line_is_concise() {
        let mut tracker = Tracker::default();
        tracker
            .ingest_summary(sample_summary())
            .expect("summary ingested");
        let records = tracker
            .ingest_trade(sample_trade())
            .expect("trade ingested");
        let line = render_human_line(&records[0]);
        assert!(line.starts_with("fill latency_ms=1250"));
        assert!(line.contains("price_gap_bps=200.0000"));
        assert!(line.contains("shares=6.00"));
        assert!(line.contains("leader_tx=0xleader-tx"));
    }

    #[test]
    fn enrich_run_error_explains_private_key_requirement() {
        let message = enrich_run_error("missing private key");
        assert!(message.contains("authenticated user websocket"));
        assert!(message.contains("PRIVATE_KEY or CLOB_PRIVATE_KEY"));
    }

    #[test]
    fn enrich_run_error_explains_clob_secret_requirement() {
        let message = enrich_run_error("missing CLOB_SECRET for websocket auth");
        assert!(message.contains("CLOB_API_KEY"));
        assert!(message.contains("CLOB_SECRET"));
    }
}
