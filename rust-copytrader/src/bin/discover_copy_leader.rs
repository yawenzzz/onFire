use rust_copytrader::wallet_filter::{
    ActivityRecord, LeaderboardEntry, MarketRecord, PositionRecord, WalletCandidateSeed,
    WalletScoreCard, build_candidate_seeds, choose_wallet, evaluate_candidate, json_objects,
    now_unix_secs, parse_activity_records, parse_leaderboard_entries, parse_market_record,
    parse_position_records, parse_total_value, parse_traded_count, render_selected_leader_env,
    render_wallet_filter_rejection_report, render_wallet_filter_report, resolve_category_scope,
};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::Duration;

const LEADERBOARD_BASE_URL: &str = "https://data-api.polymarket.com/v1/leaderboard";
const ACTIVITY_BASE_URL: &str = "https://data-api.polymarket.com/activity";
const POSITIONS_BASE_URL: &str = "https://data-api.polymarket.com/positions";
const VALUE_BASE_URL: &str = "https://data-api.polymarket.com/value";
const TRADED_BASE_URL: &str = "https://data-api.polymarket.com/traded";
const MARKET_BASE_URL: &str = "https://gamma-api.polymarket.com/markets/slug";
const ACTIVITY_PAGE_LIMIT: usize = 500;
const MAX_ACTIVITY_PAGES: usize = 20;
const MAX_ACTIVITY_OFFSET_EXCLUSIVE: usize = 3_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    leaderboard_base_url: String,
    activity_base_url: String,
    positions_base_url: String,
    value_base_url: String,
    traded_base_url: String,
    market_base_url: String,
    category: String,
    limit: usize,
    offset: usize,
    index: usize,
    activity_type: String,
    discovery_dir: String,
    curl_bin: String,
    proxy: Option<String>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    retry_count: usize,
    retry_delay_ms: u64,
    skip_activity: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            leaderboard_base_url: env::var("POLYMARKET_LEADERBOARD_BASE_URL")
                .unwrap_or_else(|_| LEADERBOARD_BASE_URL.to_string()),
            activity_base_url: env::var("POLYMARKET_ACTIVITY_BASE_URL")
                .unwrap_or_else(|_| ACTIVITY_BASE_URL.to_string()),
            positions_base_url: env::var("POLYMARKET_POSITIONS_BASE_URL")
                .unwrap_or_else(|_| POSITIONS_BASE_URL.to_string()),
            value_base_url: env::var("POLYMARKET_VALUE_BASE_URL")
                .unwrap_or_else(|_| VALUE_BASE_URL.to_string()),
            traded_base_url: env::var("POLYMARKET_TRADED_BASE_URL")
                .unwrap_or_else(|_| TRADED_BASE_URL.to_string()),
            market_base_url: env::var("POLYMARKET_MARKET_BASE_URL")
                .unwrap_or_else(|_| MARKET_BASE_URL.to_string()),
            category: "SPECIALIST".to_string(),
            limit: 25,
            offset: 0,
            index: 0,
            activity_type: "TRADE".to_string(),
            discovery_dir: "../.omx/discovery".to_string(),
            curl_bin: "curl".to_string(),
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            connect_timeout_ms: 1_500,
            max_time_ms: 8_000,
            retry_count: 1,
            retry_delay_ms: 500,
            skip_activity: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveryArtifacts {
    selected_wallet: String,
    leaderboard_path: PathBuf,
    activity_path: PathBuf,
    positions_path: PathBuf,
    value_path: PathBuf,
    traded_path: PathBuf,
    filter_report_path: PathBuf,
    selected_leader_env_path: PathBuf,
    selected_category: String,
    selected_score: i64,
    selected_rank: Option<String>,
    selected_week_rank: Option<String>,
    selected_month_rank: Option<String>,
    selected_all_rank: Option<String>,
    selected_pnl: Option<String>,
    selected_username: Option<String>,
    latest_activity_timestamp: Option<String>,
    latest_activity_side: Option<String>,
    latest_activity_slug: Option<String>,
    latest_activity_tx: Option<String>,
}

#[derive(Debug, Clone)]
struct CandidateData {
    activities: Vec<ActivityRecord>,
    positions: Vec<PositionRecord>,
    total_value: Option<f64>,
    traded_markets: u64,
    markets: BTreeMap<String, MarketRecord>,
    activity_path: PathBuf,
    positions_path: PathBuf,
    value_path: PathBuf,
    traded_path: PathBuf,
}

#[derive(Debug, Clone)]
struct CandidateBoardPaths {
    month_pnl_path: PathBuf,
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

    match execute(&options) {
        Ok(artifacts) => {
            println!("selected_wallet={}", artifacts.selected_wallet);
            println!("selected_category={}", artifacts.selected_category);
            println!("selected_score={}", artifacts.selected_score);
            println!("leaderboard_path={}", artifacts.leaderboard_path.display());
            println!("activity_path={}", artifacts.activity_path.display());
            println!("positions_path={}", artifacts.positions_path.display());
            println!("value_path={}", artifacts.value_path.display());
            println!("traded_path={}", artifacts.traded_path.display());
            println!(
                "filter_report_path={}",
                artifacts.filter_report_path.display()
            );
            if let Some(rank) = artifacts.selected_rank {
                println!("selected_rank={rank}");
            }
            if let Some(rank) = artifacts.selected_week_rank {
                println!("selected_week_rank={rank}");
            }
            if let Some(rank) = artifacts.selected_month_rank {
                println!("selected_month_rank={rank}");
            }
            if let Some(rank) = artifacts.selected_all_rank {
                println!("selected_all_rank={rank}");
            }
            if let Some(pnl) = artifacts.selected_pnl {
                println!("selected_pnl={pnl}");
            }
            if let Some(username) = artifacts.selected_username {
                println!("selected_username={username}");
            }
            if let Some(timestamp) = artifacts.latest_activity_timestamp {
                println!("latest_activity_timestamp={timestamp}");
            }
            if let Some(side) = artifacts.latest_activity_side {
                println!("latest_activity_side={side}");
            }
            if let Some(slug) = artifacts.latest_activity_slug {
                println!("latest_activity_slug={slug}");
            }
            if let Some(tx) = artifacts.latest_activity_tx {
                println!("latest_activity_tx={tx}");
            }
            println!(
                "selected_leader_env_path={}",
                artifacts.selected_leader_env_path.display()
            );
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
        "usage: discover_copy_leader [--leaderboard-base-url <url>] [--activity-base-url <url>] [--positions-base-url <url>] [--value-base-url <url>] [--traded-base-url <url>] [--market-base-url <url>] [--category <SPECIALIST|CSV|single-category>] [--limit <n>] [--offset <n>] [--index <n>] [--activity-type <value>] [--discovery-dir <path>] [--curl-bin <path>] [--proxy <url>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--retry-count <n>] [--retry-delay-ms <n>] [--time-period <ignored>] [--order-by <ignored>] [--skip-activity]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--leaderboard-base-url" => options.leaderboard_base_url = next_value(&mut iter, arg)?,
            "--activity-base-url" => options.activity_base_url = next_value(&mut iter, arg)?,
            "--positions-base-url" => options.positions_base_url = next_value(&mut iter, arg)?,
            "--value-base-url" => options.value_base_url = next_value(&mut iter, arg)?,
            "--traded-base-url" => options.traded_base_url = next_value(&mut iter, arg)?,
            "--market-base-url" => options.market_base_url = next_value(&mut iter, arg)?,
            "--category" => options.category = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--offset" => options.offset = parse_usize(&next_value(&mut iter, arg)?, "offset")?,
            "--index" => options.index = parse_usize(&next_value(&mut iter, arg)?, "index")?,
            "--activity-type" => options.activity_type = next_value(&mut iter, arg)?,
            "--discovery-dir" => options.discovery_dir = next_value(&mut iter, arg)?,
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
            "--skip-activity" => options.skip_activity = true,
            "--time-period" | "--order-by" => {
                let _ = next_value(&mut iter, arg)?;
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

fn execute(options: &Options) -> Result<DiscoveryArtifacts, String> {
    if options.skip_activity {
        return Err(
            "wallet_filter_v1 requires activity history; --skip-activity is not supported"
                .to_string(),
        );
    }

    let discovery_dir = PathBuf::from(&options.discovery_dir);
    let markets_dir = discovery_dir.join("markets");
    fs::create_dir_all(&discovery_dir)
        .map_err(|error| format!("failed to create {}: {error}", discovery_dir.display()))?;
    fs::create_dir_all(&markets_dir)
        .map_err(|error| format!("failed to create {}: {error}", markets_dir.display()))?;

    let categories = resolve_category_scope(&options.category);
    if categories.is_empty() {
        return Err("wallet_filter_v1 requires at least one non-empty category scope".to_string());
    }

    let mut candidate_seeds = Vec::<WalletCandidateSeed>::new();
    let mut board_paths = HashMap::<String, CandidateBoardPaths>::new();
    for category in &categories {
        let week = fetch_leaderboard_snapshot(options, &discovery_dir, category, "WEEK", "PNL")?;
        let month = fetch_leaderboard_snapshot(options, &discovery_dir, category, "MONTH", "PNL")?;
        let all = fetch_leaderboard_snapshot(options, &discovery_dir, category, "ALL", "PNL")?;
        let vol = fetch_leaderboard_snapshot(options, &discovery_dir, category, "MONTH", "VOL")?;
        board_paths.insert(
            category.clone(),
            CandidateBoardPaths {
                month_pnl_path: month.path.clone(),
            },
        );
        candidate_seeds.extend(build_candidate_seeds(
            &week.entries,
            &month.entries,
            &all.entries,
            &vol.entries,
        ));
    }

    if candidate_seeds.is_empty() {
        return Err(format!(
            "wallet_filter_v1 found no week/month intersection candidates under {}",
            categories.join(",")
        ));
    }

    let now_ts = now_unix_secs();
    let mut market_cache = BTreeMap::<String, MarketRecord>::new();
    let mut candidate_data = HashMap::<String, CandidateData>::new();
    let mut cards = Vec::<WalletScoreCard>::new();
    for seed in &candidate_seeds {
        let data = load_candidate_data(
            options,
            &discovery_dir,
            &markets_dir,
            &mut market_cache,
            seed,
            now_ts,
        )?;
        let card = evaluate_candidate(
            seed,
            &data.activities,
            &data.positions,
            data.total_value,
            data.traded_markets,
            &data.markets,
            now_ts,
        );
        candidate_data.insert(seed.wallet.clone(), data);
        cards.push(card);
    }

    let selection = choose_wallet(&cards, options.index).ok_or_else(|| {
        let report = render_wallet_filter_rejection_report(
            &cards,
            &format!("wallet_filter_v1:{}", categories.join(",")),
        );
        let report_path = discovery_dir.join("wallet-filter-v1-report.txt");
        let _ = write_output_file(&report_path, report.as_bytes());
        let selected_leader_env_path = discovery_dir.join("selected-leader.env");
        let _ = fs::remove_file(&selected_leader_env_path);
        format!(
            "wallet_filter_v1 rejected every candidate; see {}",
            report_path.display()
        )
    })?;

    let selected = &selection.selected;
    let selected_data = candidate_data
        .get(&selected.seed.wallet)
        .ok_or_else(|| "selected candidate data missing".to_string())?;
    let leaderboard_path = board_paths
        .get(&selected.seed.category)
        .map(|paths| paths.month_pnl_path.clone())
        .ok_or_else(|| "selected leaderboard artifact missing".to_string())?;

    let report_path = discovery_dir.join("wallet-filter-v1-report.txt");
    let report_source = format!(
        "wallet_filter_v1:{}#{}",
        selected.seed.category, options.index
    );
    write_output_file(
        &report_path,
        render_wallet_filter_report(&selection, &report_source).as_bytes(),
    )
    .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;

    let selected_leader_env_path = discovery_dir.join("selected-leader.env");
    write_output_file(
        &selected_leader_env_path,
        render_selected_leader_env(&selection, &report_source).as_bytes(),
    )
    .map_err(|error| {
        format!(
            "failed to write {}: {error}",
            selected_leader_env_path.display()
        )
    })?;

    Ok(DiscoveryArtifacts {
        selected_wallet: selected.seed.wallet.clone(),
        leaderboard_path,
        activity_path: selected_data.activity_path.clone(),
        positions_path: selected_data.positions_path.clone(),
        value_path: selected_data.value_path.clone(),
        traded_path: selected_data.traded_path.clone(),
        filter_report_path: report_path,
        selected_leader_env_path,
        selected_category: selected.seed.category.clone(),
        selected_score: selected.score_total,
        selected_rank: selected.seed.month_rank.map(|value| value.to_string()),
        selected_week_rank: selected.seed.week_rank.map(|value| value.to_string()),
        selected_month_rank: selected.seed.month_rank.map(|value| value.to_string()),
        selected_all_rank: selected.seed.all_rank.map(|value| value.to_string()),
        selected_pnl: Some(format!("{:.6}", selected.seed.month_pnl)),
        selected_username: selected.seed.username.clone(),
        latest_activity_timestamp: selected.metrics.latest_trade.timestamp.clone(),
        latest_activity_side: selected.metrics.latest_trade.side.clone(),
        latest_activity_slug: selected.metrics.latest_trade.slug.clone(),
        latest_activity_tx: selected.metrics.latest_trade.tx.clone(),
    })
}

fn load_candidate_data(
    options: &Options,
    discovery_dir: &Path,
    markets_dir: &Path,
    market_cache: &mut BTreeMap<String, MarketRecord>,
    seed: &WalletCandidateSeed,
    now_ts: u64,
) -> Result<CandidateData, String> {
    let activity_path = discovery_dir.join(format!(
        "activity-{}-90d.json",
        sanitize_for_filename(&seed.wallet)
    ));
    let activities_body = fetch_activity_history(options, &seed.wallet, now_ts)?;
    write_output_file(&activity_path, activities_body.as_bytes()).map_err(|error| {
        format!(
            "failed to write activity artifact {}: {error}",
            activity_path.display()
        )
    })?;
    let activities = parse_activity_records(&activities_body);

    let positions_path = discovery_dir.join(format!(
        "positions-{}.json",
        sanitize_for_filename(&seed.wallet)
    ));
    let positions_body = fetch_simple_json(
        &build_positions_url(&options.positions_base_url, &seed.wallet),
        options,
    )?;
    write_output_file(&positions_path, positions_body.as_bytes()).map_err(|error| {
        format!(
            "failed to write positions artifact {}: {error}",
            positions_path.display()
        )
    })?;
    let positions = parse_position_records(&positions_body);

    let value_path = discovery_dir.join(format!(
        "value-{}.json",
        sanitize_for_filename(&seed.wallet)
    ));
    let value_body = fetch_simple_json(
        &build_value_url(&options.value_base_url, &seed.wallet),
        options,
    )?;
    write_output_file(&value_path, value_body.as_bytes()).map_err(|error| {
        format!(
            "failed to write value artifact {}: {error}",
            value_path.display()
        )
    })?;
    let total_value = parse_total_value(&value_body);

    let traded_path = discovery_dir.join(format!(
        "traded-{}.json",
        sanitize_for_filename(&seed.wallet)
    ));
    let traded_body = fetch_simple_json(
        &build_traded_url(&options.traded_base_url, &seed.wallet),
        options,
    )?;
    write_output_file(&traded_path, traded_body.as_bytes()).map_err(|error| {
        format!(
            "failed to write traded artifact {}: {error}",
            traded_path.display()
        )
    })?;
    let traded_markets = parse_traded_count(&traded_body).unwrap_or(0);

    let mut markets = BTreeMap::<String, MarketRecord>::new();
    let market_slugs = collect_market_slugs(&activities, &positions);
    for slug in market_slugs {
        if let Some(market) = market_cache.get(&slug) {
            markets.insert(slug.clone(), market.clone());
            continue;
        }
        let path = markets_dir.join(format!("{}.json", sanitize_for_filename(&slug)));
        let body = if path.exists() {
            fs::read_to_string(&path).map_err(|error| {
                format!("failed to read market artifact {}: {error}", path.display())
            })?
        } else {
            let body =
                fetch_simple_json(&build_market_url(&options.market_base_url, &slug), options)?;
            write_output_file(&path, body.as_bytes()).map_err(|error| {
                format!(
                    "failed to write market artifact {}: {error}",
                    path.display()
                )
            })?;
            body
        };
        if let Some(market) = parse_market_record(&body, &slug) {
            market_cache.insert(slug.clone(), market.clone());
            markets.insert(slug, market);
        }
    }

    Ok(CandidateData {
        activities,
        positions,
        total_value,
        traded_markets,
        markets,
        activity_path,
        positions_path,
        value_path,
        traded_path,
    })
}

fn collect_market_slugs(
    activities: &[ActivityRecord],
    positions: &[PositionRecord],
) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    for activity in activities {
        if let Some(slug) = activity
            .slug
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            seen.insert(slug.to_string(), ());
        }
    }
    for position in positions {
        if let Some(slug) = position
            .slug
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            seen.insert(slug.to_string(), ());
        }
    }
    seen.into_keys().collect()
}

fn fetch_activity_history(options: &Options, wallet: &str, now_ts: u64) -> Result<String, String> {
    let start_ts = now_ts.saturating_sub(rust_copytrader::wallet_filter::LOOKBACK_SECS);
    let mut offset = 0;
    let mut objects = Vec::<String>::new();
    for _ in 0..MAX_ACTIVITY_PAGES {
        if offset >= MAX_ACTIVITY_OFFSET_EXCLUSIVE {
            break;
        }
        let url = build_activity_url(
            &options.activity_base_url,
            wallet,
            start_ts,
            now_ts,
            ACTIVITY_PAGE_LIMIT,
            offset,
        );
        let body = fetch_simple_json(&url, options)?;
        let page_objects = json_objects(&body);
        if page_objects.is_empty() {
            break;
        }
        objects.extend(page_objects.iter().cloned());
        if page_objects.len() < ACTIVITY_PAGE_LIMIT {
            break;
        }
        offset += ACTIVITY_PAGE_LIMIT;
    }
    Ok(format!("[{}]", objects.join(",")))
}

fn fetch_leaderboard_snapshot(
    options: &Options,
    discovery_dir: &Path,
    category: &str,
    time_period: &str,
    order_by: &str,
) -> Result<LeaderboardSnapshot, String> {
    let path = discovery_dir.join(format!(
        "leaderboard-{}-{}-{}.json",
        sanitize_for_filename(category),
        sanitize_for_filename(time_period),
        sanitize_for_filename(order_by)
    ));
    let body = fetch_simple_json(
        &build_leaderboard_url(
            &options.leaderboard_base_url,
            category,
            time_period,
            order_by,
            options.limit,
            options.offset,
        ),
        options,
    )?;
    write_output_file(&path, body.as_bytes()).map_err(|error| {
        format!(
            "failed to write leaderboard artifact {}: {error}",
            path.display()
        )
    })?;
    Ok(LeaderboardSnapshot {
        path,
        entries: parse_leaderboard_entries(&body, category, time_period, order_by),
    })
}

fn fetch_simple_json(url: &str, options: &Options) -> Result<String, String> {
    let output = run_request_with_retry(
        &options.curl_bin,
        &build_curl_args(
            url,
            options.proxy.as_deref(),
            options.connect_timeout_ms,
            options.max_time_ms,
        ),
        options.retry_count,
        options.retry_delay_ms,
    )
    .map_err(|error| format!("{url} -> {error}"))?;
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{url} -> response was not utf-8: {error}"))
}

fn build_leaderboard_url(
    base_url: &str,
    category: &str,
    time_period: &str,
    order_by: &str,
    limit: usize,
    offset: usize,
) -> String {
    format!(
        "{}?category={}&timePeriod={}&orderBy={}&limit={}&offset={}",
        base_url.trim_end_matches('/'),
        encode_component(category),
        encode_component(time_period),
        encode_component(order_by),
        limit,
        offset
    )
}

fn build_activity_url(
    base_url: &str,
    wallet: &str,
    start_ts: u64,
    end_ts: u64,
    limit: usize,
    offset: usize,
) -> String {
    format!(
        "{}?user={}&limit={}&offset={}&sortBy=TIMESTAMP&sortDirection=DESC&start={}&end={}",
        base_url.trim_end_matches('/'),
        encode_component(wallet),
        limit,
        offset,
        start_ts,
        end_ts,
    )
}

fn build_positions_url(base_url: &str, wallet: &str) -> String {
    format!(
        "{}?user={}&limit=500&offset=0&sortBy=CURRENT&sortDirection=DESC",
        base_url.trim_end_matches('/'),
        encode_component(wallet)
    )
}

fn build_value_url(base_url: &str, wallet: &str) -> String {
    format!(
        "{}?user={}",
        base_url.trim_end_matches('/'),
        encode_component(wallet)
    )
}

fn build_traded_url(base_url: &str, wallet: &str) -> String {
    format!(
        "{}?user={}",
        base_url.trim_end_matches('/'),
        encode_component(wallet)
    )
}

fn build_market_url(base_url: &str, slug: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        encode_component(slug)
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
        seconds_from_ms(connect_timeout_ms),
        "--max-time".to_string(),
        seconds_from_ms(max_time_ms),
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

fn seconds_from_ms(value: u64) -> String {
    format!("{:.3}", value as f64 / 1_000.0)
}

fn run_request(curl_bin: &str, args: &[String]) -> Result<Output, String> {
    let output = Command::new(curl_bin)
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute {curl_bin}: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "{} exited with {}: {}{}",
            curl_bin,
            output.status.code().unwrap_or(1),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", stdout.trim())
            }
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

fn write_output_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
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

#[derive(Debug, Clone)]
struct LeaderboardSnapshot {
    path: PathBuf,
    entries: Vec<LeaderboardEntry>,
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVITY_PAGE_LIMIT, build_activity_url, build_leaderboard_url, execute,
        is_retryable_transport_error, parse_args, seconds_from_ms,
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
        std::env::temp_dir().join(format!("discover-copy-leader-v1-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_wallet_filter_v1_options() {
        let options = parse_args(&[
            "--category".into(),
            "SPORTS,CRYPTO".into(),
            "--limit".into(),
            "10".into(),
            "--offset".into(),
            "5".into(),
            "--index".into(),
            "1".into(),
            "--activity-type".into(),
            "TRADE".into(),
            "--positions-base-url".into(),
            "https://example.com/positions".into(),
            "--market-base-url".into(),
            "https://example.com/markets/slug".into(),
        ])
        .expect("parse");

        assert_eq!(options.category, "SPORTS,CRYPTO");
        assert_eq!(options.limit, 10);
        assert_eq!(options.offset, 5);
        assert_eq!(options.index, 1);
        assert_eq!(options.activity_type, "TRADE");
        assert_eq!(options.positions_base_url, "https://example.com/positions");
        assert_eq!(options.market_base_url, "https://example.com/markets/slug");
    }

    #[test]
    fn urls_follow_expected_shape() {
        let leaderboard = build_leaderboard_url(
            "https://data-api.polymarket.com/v1/leaderboard",
            "SPORTS",
            "MONTH",
            "PNL",
            25,
            0,
        );
        let activity = build_activity_url(
            "https://data-api.polymarket.com/activity",
            "0xwallet",
            100,
            200,
            ACTIVITY_PAGE_LIMIT,
            0,
        );
        assert!(leaderboard.contains("category=SPORTS"));
        assert!(leaderboard.contains("timePeriod=MONTH"));
        assert!(leaderboard.contains("orderBy=PNL"));
        assert!(activity.contains("user=0xwallet"));
        assert!(activity.contains("start=100"));
        assert!(activity.contains("end=200"));
    }

    #[test]
    fn execute_runs_wallet_filter_v1_and_writes_artifacts() {
        let root = unique_temp_dir("execute");
        fs::create_dir_all(&root).expect("temp dir created");
        let curl_stub = root.join("curl-stub.sh");
        fs::write(
            &curl_stub,
            concat!(
                "#!/usr/bin/env bash\n",
                "url=\"${@: -1}\"\n",
                "if [[ \"$url\" == *\"leaderboard\"* && \"$url\" == *\"category=SPORTS\"* && \"$url\" == *\"timePeriod=WEEK\"* && \"$url\" == *\"orderBy=PNL\"* ]]; then\n",
                "  printf '[{\"rank\":\"1\",\"proxyWallet\":\"0xgood\",\"userName\":\"good\",\"vol\":10000,\"pnl\":800},{\"rank\":\"2\",\"proxyWallet\":\"0xbad\",\"userName\":\"bad\",\"vol\":20000,\"pnl\":700}]'\n",
                "elif [[ \"$url\" == *\"leaderboard\"* && \"$url\" == *\"category=SPORTS\"* && \"$url\" == *\"timePeriod=MONTH\"* && \"$url\" == *\"orderBy=PNL\"* ]]; then\n",
                "  printf '[{\"rank\":\"2\",\"proxyWallet\":\"0xgood\",\"userName\":\"good\",\"vol\":20000,\"pnl\":1600},{\"rank\":\"3\",\"proxyWallet\":\"0xbad\",\"userName\":\"bad\",\"vol\":30000,\"pnl\":1400}]'\n",
                "elif [[ \"$url\" == *\"leaderboard\"* && \"$url\" == *\"category=SPORTS\"* && \"$url\" == *\"timePeriod=ALL\"* && \"$url\" == *\"orderBy=PNL\"* ]]; then\n",
                "  printf '[{\"rank\":\"5\",\"proxyWallet\":\"0xgood\",\"userName\":\"good\",\"vol\":40000,\"pnl\":5000}]'\n",
                "elif [[ \"$url\" == *\"leaderboard\"* && \"$url\" == *\"category=SPORTS\"* && \"$url\" == *\"timePeriod=MONTH\"* && \"$url\" == *\"orderBy=VOL\"* ]]; then\n",
                "  printf '[{\"rank\":\"1\",\"proxyWallet\":\"0xbad\",\"userName\":\"bad\",\"vol\":50000,\"pnl\":100}]'\n",
                "elif [[ \"$url\" == *\"activity\"*\"user=0xgood\"* ]]; then\n",
                "  printf '[",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770000000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg1\",\"asset\":\"a1\",\"side\":\"BUY\",\"slug\":\"market-1\",\"conditionId\":\"c1\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770086400,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg2\",\"asset\":\"a1\",\"side\":\"SELL\",\"slug\":\"market-1\",\"conditionId\":\"c1\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770100000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg3\",\"asset\":\"a2\",\"side\":\"BUY\",\"slug\":\"market-2\",\"conditionId\":\"c2\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770300000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg4\",\"asset\":\"a2\",\"side\":\"SELL\",\"slug\":\"market-2\",\"conditionId\":\"c2\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770310000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg5\",\"asset\":\"a3\",\"side\":\"BUY\",\"slug\":\"market-3\",\"conditionId\":\"c3\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770500000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg6\",\"asset\":\"a3\",\"side\":\"SELL\",\"slug\":\"market-3\",\"conditionId\":\"c3\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770510000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg7\",\"asset\":\"a4\",\"side\":\"BUY\",\"slug\":\"market-4\",\"conditionId\":\"c4\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770700000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg8\",\"asset\":\"a4\",\"side\":\"SELL\",\"slug\":\"market-4\",\"conditionId\":\"c4\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770710000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg9\",\"asset\":\"a5\",\"side\":\"BUY\",\"slug\":\"market-5\",\"conditionId\":\"c5\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770900000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg10\",\"asset\":\"a5\",\"side\":\"SELL\",\"slug\":\"market-5\",\"conditionId\":\"c5\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1770910000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg11\",\"asset\":\"a6\",\"side\":\"BUY\",\"slug\":\"market-6\",\"conditionId\":\"c6\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1771100000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg12\",\"asset\":\"a6\",\"side\":\"SELL\",\"slug\":\"market-6\",\"conditionId\":\"c6\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1771110000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg13\",\"asset\":\"a7\",\"side\":\"BUY\",\"slug\":\"market-7\",\"conditionId\":\"c7\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1771300000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg14\",\"asset\":\"a7\",\"side\":\"SELL\",\"slug\":\"market-7\",\"conditionId\":\"c7\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1771310000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg15\",\"asset\":\"a8\",\"side\":\"BUY\",\"slug\":\"market-8\",\"conditionId\":\"c8\"},",
                "{\"proxyWallet\":\"0xgood\",\"timestamp\":1771500000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xg16\",\"asset\":\"a8\",\"side\":\"SELL\",\"slug\":\"market-8\",\"conditionId\":\"c8\"}]'\n",
                "elif [[ \"$url\" == *\"activity\"*\"user=0xbad\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xbad\",\"timestamp\":1770000000,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xb1\",\"asset\":\"b1\",\"side\":\"BUY\",\"slug\":\"market-1\",\"conditionId\":\"c1\"},{\"proxyWallet\":\"0xbad\",\"timestamp\":1770000100,\"type\":\"TRADE\",\"size\":10,\"usdcSize\":100,\"transactionHash\":\"0xb2\",\"asset\":\"b1\",\"side\":\"SELL\",\"slug\":\"market-1\",\"conditionId\":\"c1\"},{\"proxyWallet\":\"0xbad\",\"timestamp\":1770000200,\"type\":\"MAKER_REBATE\",\"size\":0,\"usdcSize\":1,\"asset\":\"b1\"}]'\n",
                "elif [[ \"$url\" == *\"positions\"*\"user=0xgood\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xgood\",\"asset\":\"a1\",\"conditionId\":\"c1\",\"slug\":\"market-1\",\"currentValue\":500,\"endDate\":\"2026-12-31T00:00:00Z\",\"negativeRisk\":false}]'\n",
                "elif [[ \"$url\" == *\"positions\"*\"user=0xbad\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xbad\",\"asset\":\"b1\",\"conditionId\":\"c1\",\"slug\":\"market-1\",\"currentValue\":5,\"endDate\":\"2026-12-31T00:00:00Z\",\"negativeRisk\":false}]'\n",
                "elif [[ \"$url\" == *\"/value\"*\"user=0xgood\"* ]]; then\n",
                "  printf '[{\"user\":\"0xgood\",\"value\":500}]'\n",
                "elif [[ \"$url\" == *\"/value\"*\"user=0xbad\"* ]]; then\n",
                "  printf '[{\"user\":\"0xbad\",\"value\":5}]'\n",
                "elif [[ \"$url\" == *\"/traded\"*\"user=0xgood\"* ]]; then\n",
                "  printf '{\"user\":\"0xgood\",\"traded\":25}'\n",
                "elif [[ \"$url\" == *\"/traded\"*\"user=0xbad\"* ]]; then\n",
                "  printf '{\"user\":\"0xbad\",\"traded\":5}'\n",
                "elif [[ \"$url\" == *\"markets/slug/market-\"* ]]; then\n",
                "  printf '{\"slug\":\"market-generic\",\"conditionId\":\"c\",\"category\":\"SPORTS\",\"endDate\":\"2026-12-31T00:00:00Z\",\"acceptingOrders\":true,\"enableOrderBook\":true,\"liquidityClob\":90000,\"volume24hrClob\":50000,\"negRisk\":false}'\n",
                "else\n",
                "  echo \"unhandled url: $url\" >&2\n",
                "  exit 1\n",
                "fi\n"
            ),
        )
        .expect("stub written");
        let mut perms = fs::metadata(&curl_stub).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&curl_stub, perms).expect("perms");

        let options = parse_args(&[
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--discovery-dir".into(),
            root.join("discovery").display().to_string(),
            "--category".into(),
            "SPORTS".into(),
        ])
        .expect("parse");

        let artifacts = execute(&options).expect("execute should succeed");

        assert_eq!(artifacts.selected_wallet, "0xgood");
        assert_eq!(artifacts.selected_category, "SPORTS");
        assert!(artifacts.activity_path.exists());
        assert!(artifacts.positions_path.exists());
        assert!(artifacts.value_path.exists());
        assert!(artifacts.traded_path.exists());
        assert!(artifacts.filter_report_path.exists());
        assert!(artifacts.selected_leader_env_path.exists());
        let report = fs::read_to_string(&artifacts.filter_report_path).expect("report");
        assert!(report.contains("wallet_filter_strategy=wallet_filter_v1"));
        assert!(report.contains("selected_wallet=0xgood"));
        let env = fs::read_to_string(&artifacts.selected_leader_env_path).expect("env");
        assert!(env.contains("COPYTRADER_SELECTED_CATEGORY=SPORTS"));
        assert!(env.contains("COPYTRADER_SELECTED_SCORE="));
        assert!(env.contains("COPYTRADER_FILTER_COPYABLE_RATIO="));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn execute_clears_stale_selected_env_when_every_candidate_is_rejected() {
        let root = unique_temp_dir("reject-all");
        let discovery_dir = root.join("discovery");
        fs::create_dir_all(&discovery_dir).expect("discovery dir created");
        fs::write(
            discovery_dir.join("selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xstale\n",
        )
        .expect("stale env written");

        let curl_stub = root.join("curl-stub.sh");
        fs::write(
            &curl_stub,
            concat!(
                "#!/usr/bin/env bash\n",
                "url=\"${@: -1}\"\n",
                "if [[ \"$url\" == *\"leaderboard\"* ]]; then\n",
                "  printf '[{\"rank\":\"1\",\"proxyWallet\":\"0xbad\",\"userName\":\"bad\",\"vol\":1000,\"pnl\":50}]'\n",
                "elif [[ \"$url\" == *\"activity\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xbad\",\"timestamp\":1770000000,\"type\":\"MAKER_REBATE\",\"size\":0,\"usdcSize\":1,\"asset\":\"b1\"}]'\n",
                "elif [[ \"$url\" == *\"positions\"* ]]; then\n",
                "  printf '[{\"proxyWallet\":\"0xbad\",\"asset\":\"b1\",\"conditionId\":\"c1\",\"slug\":\"market-1\",\"currentValue\":0,\"endDate\":\"2026-12-31T00:00:00Z\",\"negativeRisk\":false}]'\n",
                "elif [[ \"$url\" == *\"/value\"* ]]; then\n",
                "  printf '[{\"user\":\"0xbad\",\"value\":0}]'\n",
                "elif [[ \"$url\" == *\"/traded\"* ]]; then\n",
                "  printf '{\"user\":\"0xbad\",\"traded\":1}'\n",
                "elif [[ \"$url\" == *\"markets/slug/market-1\"* ]]; then\n",
                "  printf '{\"slug\":\"market-1\",\"conditionId\":\"c1\",\"category\":\"SPORTS\",\"endDate\":\"2026-12-31T00:00:00Z\",\"acceptingOrders\":true,\"enableOrderBook\":true,\"liquidityClob\":90000,\"volume24hrClob\":50000,\"negRisk\":false}'\n",
                "else\n",
                "  echo \"unhandled url: $url\" >&2\n",
                "  exit 1\n",
                "fi\n"
            ),
        )
        .expect("stub written");
        let mut perms = fs::metadata(&curl_stub).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&curl_stub, perms).expect("perms");

        let options = parse_args(&[
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--discovery-dir".into(),
            discovery_dir.display().to_string(),
            "--category".into(),
            "SPORTS".into(),
        ])
        .expect("parse");

        let error = execute(&options).expect_err("all candidates should be rejected");
        assert!(error.contains("wallet_filter_v1 rejected every candidate"));
        assert!(!discovery_dir.join("selected-leader.env").exists());
        let report = fs::read_to_string(discovery_dir.join("wallet-filter-v1-report.txt"))
            .expect("report should exist");
        assert!(report.contains("selected_wallet=none"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn retryable_transport_errors_cover_timeout_and_ssl_failures() {
        assert!(is_retryable_transport_error(
            "curl exited with 28: curl: (28) timeout"
        ));
        assert!(is_retryable_transport_error(
            "curl exited with 35: curl: (35) SSL_ERROR_SYSCALL"
        ));
        assert!(!is_retryable_transport_error(
            "curl exited with 22: HTTP 404"
        ));
    }

    #[test]
    fn seconds_from_ms_formats_fractional_seconds() {
        assert_eq!(seconds_from_ms(1500), "1.500");
        assert_eq!(seconds_from_ms(8000), "8.000");
    }
}
