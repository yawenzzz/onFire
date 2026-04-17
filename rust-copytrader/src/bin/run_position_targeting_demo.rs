use rust_copytrader::adapters::positions::{
    build_leader_state, parse_leader_positions_payload, parse_total_value_payload,
};
use rust_copytrader::domain::position_targeting::{
    AssetId, BookLevel, BookView, LeaderConfig, MarketMeta, OwnPosition, PricePpm, SizingInput,
    StrategyConfig, compute_targets,
};
use rust_copytrader::wallet_filter::parse_market_record;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    discovery_dir: Option<String>,
    selected_leader_env: Option<String>,
    equity_usdc: i64,
    ewma_beta_bps: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            discovery_dir: None,
            selected_leader_env: None,
            equity_usdc: 1_000_000_000,
            ewma_beta_bps: 2_500,
        }
    }
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

    match run_demo(&options) {
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
        "usage: run_position_targeting_demo [--root <path>] [--discovery-dir <path>] [--selected-leader-env <path>] [--equity-usdc <micros>] [--ewma-beta-bps <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--discovery-dir" => options.discovery_dir = Some(next_value(&mut iter, arg)?),
            "--selected-leader-env" => {
                options.selected_leader_env = Some(next_value(&mut iter, arg)?)
            }
            "--equity-usdc" => {
                options.equity_usdc = parse_i64(&next_value(&mut iter, arg)?, "equity-usdc")?
            }
            "--ewma-beta-bps" => {
                options.ewma_beta_bps = parse_u32(&next_value(&mut iter, arg)?, "ewma-beta-bps")?
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

fn parse_i64(value: &str, field: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_u32(value: &str, field: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn run_demo(options: &Options) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&options.root);
    let discovery_dir = options
        .discovery_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omx/discovery"));
    let selected_env = options
        .selected_leader_env
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| discovery_dir.join("selected-leader.env"));
    let wallet = read_env_value(
        &selected_env,
        &["COPYTRADER_DISCOVERY_WALLET", "COPYTRADER_LEADER_WALLET"],
    )?;
    let positions_path =
        discovery_dir.join(format!("positions-{}.json", sanitize_for_filename(&wallet)));
    let value_path = discovery_dir.join(format!("value-{}.json", sanitize_for_filename(&wallet)));
    let positions_body = fs::read_to_string(&positions_path)
        .map_err(|error| format!("failed to read {}: {error}", positions_path.display()))?;
    let value_body = fs::read_to_string(&value_path)
        .map_err(|error| format!("failed to read {}: {error}", value_path.display()))?;

    let positions = parse_leader_positions_payload(&positions_body);
    if positions.is_empty() {
        return Err(format!(
            "no positions found in {}",
            positions_path.display()
        ));
    }
    let spot_value = parse_total_value_payload(&value_body).unwrap_or_else(|| {
        positions
            .iter()
            .map(|position| position.current_value.max(0))
            .sum::<i64>()
    });
    let base_score_bps = read_env_value(&selected_env, &["COPYTRADER_SELECTED_SCORE"])
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|value| value.saturating_mul(100))
        .unwrap_or(8_000)
        .min(10_000);

    let leader_state = build_leader_state(
        LeaderConfig {
            leader: rust_copytrader::domain::position_targeting::LeaderId(wallet.clone()),
            base_score_bps,
            alpha_bps: 3_000,
            enabled: true,
        },
        spot_value,
        None,
        positions.clone(),
        Vec::new(),
        now_ms(),
        250,
        250,
        1_000,
        options.ewma_beta_bps,
    );

    let metas = build_market_meta_map(&discovery_dir, &positions)?;
    let books = build_synthetic_books(&positions, &metas);
    let own_positions = HashMap::<AssetId, OwnPosition>::new();
    let cfg = StrategyConfig::default();
    let sizing_now_ms = now_ms();
    let output = compute_targets(&SizingInput {
        now_ms: sizing_now_ms,
        equity_usdc: options.equity_usdc,
        leaders: std::slice::from_ref(&leader_state),
        books: &books,
        metas: &metas,
        own_positions: &own_positions,
        cfg: &cfg,
    });

    let blocker_summary = summarize_blockers(&positions, &metas, &output, &cfg, sizing_now_ms);
    let report_path = write_report(
        &root,
        &wallet,
        &leader_state,
        &positions,
        &output,
        &selected_env,
        &blocker_summary,
    )?;

    let mut lines = vec![
        "mode=position-targeting-demo".to_string(),
        format!("selected_leader_wallet={wallet}"),
        format!(
            "selected_leader_rank={}",
            read_optional_env_value(&selected_env, &["COPYTRADER_SELECTED_RANK"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "selected_leader_review_status={}",
            read_optional_env_value(&selected_env, &["COPYTRADER_SELECTED_REVIEW_STATUS"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "selected_leader_core_pool_count={}",
            read_optional_env_value(&selected_env, &["COPYTRADER_CORE_POOL_COUNT"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("leader_position_count={}", positions.len()),
        format!("leader_spot_value_usdc={spot_value}"),
        format!("leader_ewma_value_usdc={}", leader_state.value.ewma_value),
        format!("target_count={}", output.targets.len()),
        format!("delta_count={}", output.deltas.len()),
        format!(
            "diagnostic_aggregated_assets={}",
            output.diagnostics.aggregated_assets
        ),
        format!(
            "diagnostic_total_target_risk_usdc={}",
            output.diagnostics.total_target_risk_usdc
        ),
        format!(
            "diagnostic_stale_asset_count={}",
            output.diagnostics.stale_assets.len()
        ),
        format!(
            "diagnostic_blocked_asset_count={}",
            output.diagnostics.blocked_assets.len()
        ),
        format!("diagnostic_blocker_summary={}", blocker_summary),
        format!("report_path={}", report_path.display()),
    ];
    for (index, target) in output.targets.iter().enumerate().take(5) {
        lines.push(format!("target[{index}].asset={}", target.asset.0));
        lines.push(format!(
            "target[{index}].risk_usdc={}",
            target.signed_target_risk_usdc
        ));
        lines.push(format!(
            "target[{index}].confidence_bps={}",
            target.confidence_bps
        ));
        lines.push(format!("target[{index}].stale={}", target.stale));
    }
    for (index, delta) in output.deltas.iter().enumerate().take(5) {
        lines.push(format!("delta[{index}].asset={}", delta.asset.0));
        lines.push(format!(
            "delta[{index}].target_risk_usdc={}",
            delta.target_risk_usdc
        ));
        lines.push(format!(
            "delta[{index}].delta_risk_usdc={}",
            delta.delta_risk_usdc
        ));
        lines.push(format!(
            "delta[{index}].confidence_bps={}",
            delta.confidence_bps
        ));
        lines.push(format!("delta[{index}].tte_bucket={:?}", delta.tte_bucket));
    }
    Ok(lines)
}

fn build_market_meta_map(
    discovery_dir: &Path,
    positions: &[rust_copytrader::domain::position_targeting::LeaderPosition],
) -> Result<HashMap<AssetId, MarketMeta>, String> {
    let mut metas = HashMap::new();
    for position in positions {
        let market_path = discovery_dir
            .join("markets")
            .join(format!("{}.json", sanitize_for_filename(&position.slug)));
        let meta = if market_path.exists() {
            let body = fs::read_to_string(&market_path)
                .map_err(|error| format!("failed to read {}: {error}", market_path.display()))?;
            parse_market_record(&body, &position.slug)
        } else {
            None
        };
        let market = meta
            .map(|record| MarketMeta {
                asset: position.asset.clone(),
                condition: position.condition.clone(),
                event: position.event.clone(),
                end_ts_ms: if position.end_ts_ms > 0 {
                    position.end_ts_ms
                } else {
                    0
                },
                accepting_orders: record.accepting_orders,
                enable_order_book: record.enable_order_book,
                liquidity_clob: record.liquidity_clob.max(0.0).round() as i64,
                volume_24h_clob: record.volume24hr_clob.max(0.0).round() as i64,
                neg_risk: record.negative_risk.unwrap_or(position.neg_risk),
            })
            .unwrap_or_else(|| MarketMeta {
                asset: position.asset.clone(),
                condition: position.condition.clone(),
                event: position.event.clone(),
                end_ts_ms: position.end_ts_ms,
                accepting_orders: true,
                enable_order_book: true,
                liquidity_clob: 100_000_000_000,
                volume_24h_clob: 100_000_000_000,
                neg_risk: position.neg_risk,
            });
        metas.insert(position.asset.clone(), market);
    }
    Ok(metas)
}

fn build_synthetic_books(
    positions: &[rust_copytrader::domain::position_targeting::LeaderPosition],
    metas: &HashMap<AssetId, MarketMeta>,
) -> HashMap<AssetId, BookView> {
    let mut books = HashMap::new();
    for position in positions {
        let price_ppm = position.avg_price_ppm.max(1);
        let ask = ((price_ppm as f64) * 1.002).round() as PricePpm;
        let bid = ((price_ppm as f64) * 0.998).round() as PricePpm;
        let size = position.size.max(1_000_000);
        let meta = metas.get(&position.asset);
        books.insert(
            position.asset.clone(),
            BookView {
                asset: position.asset.clone(),
                bids: vec![BookLevel {
                    price_ppm: bid.max(1),
                    size_shares: size,
                }],
                asks: vec![BookLevel {
                    price_ppm: ask.max(1),
                    size_shares: size,
                }],
                tick_size_ppm: 1_000,
                min_order_size_shares: 10_000,
                last_trade_price_ppm: price_ppm.max(1),
                last_update_ms: now_ms(),
                hash: format!("synthetic-{}", position.asset.0),
            },
        );
        if let Some(meta) = meta {
            let _ = meta.accepting_orders;
        }
    }
    books
}

fn summarize_blockers(
    positions: &[rust_copytrader::domain::position_targeting::LeaderPosition],
    metas: &HashMap<AssetId, MarketMeta>,
    output: &rust_copytrader::domain::position_targeting::SizingOutput,
    cfg: &StrategyConfig,
    now_ms: i64,
) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    let delta_assets = output
        .deltas
        .iter()
        .map(|delta| delta.asset.clone())
        .collect::<HashSet<_>>();
    for target in &output.targets {
        let Some(meta) = metas.get(&target.asset) else {
            *counts.entry("missing_meta".to_string()).or_default() += 1;
            continue;
        };
        let has_delta = delta_assets.contains(&target.asset);
        let tte = meta.end_ts_ms.saturating_sub(now_ms);
        if !meta.accepting_orders || !meta.enable_order_book {
            *counts.entry("closed_or_book_off".to_string()).or_default() += 1;
        }
        if tte < cfg.no_new_position_before_ms {
            *counts.entry("tail_lt24h".to_string()).or_default() += 1;
        } else if tte < cfg.reduce_size_before_ms {
            *counts.entry("tail_lt72h".to_string()).or_default() += 1;
        }
        if meta.neg_risk {
            *counts.entry("neg_risk".to_string()).or_default() += 1;
        }
        if meta.liquidity_clob < cfg.min_copyable_liquidity_usdc
            || meta.volume_24h_clob < cfg.min_copyable_volume_usdc
        {
            *counts
                .entry("low_copyable_liquidity".to_string())
                .or_default() += 1;
        }
        if target.signed_target_risk_usdc == 0 {
            *counts.entry("zero_target".to_string()).or_default() += 1;
        } else if !has_delta {
            *counts
                .entry("below_min_effective_order".to_string())
                .or_default() += 1;
        }
        if target.stale {
            *counts.entry("stale_target".to_string()).or_default() += 1;
        }
    }
    for (_, reason) in &output.diagnostics.blocked_assets {
        *counts.entry(reason.clone()).or_default() += 1;
    }
    let mut entries = counts
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    if entries.is_empty() {
        if positions.is_empty() {
            "none".to_string()
        } else {
            "none_detected".to_string()
        }
    } else {
        entries
            .into_iter()
            .take(6)
            .map(|(reason, count)| format!("{reason}:{count}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn read_env_value(path: &Path, keys: &[&str]) -> Result<String, String> {
    read_optional_env_value(path, keys)
        .ok_or_else(|| format!("missing one of {} in {}", keys.join(","), path.display()))
}

fn read_optional_env_value(path: &Path, keys: &[&str]) -> Option<String> {
    let body = fs::read_to_string(path).ok()?;
    keys.iter().find_map(|key| {
        body.lines().find_map(|line| {
            let (candidate, value) = line.split_once('=')?;
            (candidate.trim() == *key).then(|| value.trim().to_string())
        })
    })
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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn write_report(
    root: &Path,
    wallet: &str,
    leader_state: &rust_copytrader::domain::position_targeting::LeaderState,
    positions: &[rust_copytrader::domain::position_targeting::LeaderPosition],
    output: &rust_copytrader::domain::position_targeting::SizingOutput,
    selected_env: &Path,
    blocker_summary: &str,
) -> Result<PathBuf, String> {
    let report_dir = root.join(".omx/position-targeting");
    fs::create_dir_all(&report_dir)
        .map_err(|error| format!("failed to create {}: {error}", report_dir.display()))?;
    let report_path = report_dir.join(format!(
        "position-targeting-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut lines = vec![
        format!("wallet={wallet}"),
        format!(
            "selected_leader_rank={}",
            read_optional_env_value(selected_env, &["COPYTRADER_SELECTED_RANK"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "selected_leader_review_status={}",
            read_optional_env_value(selected_env, &["COPYTRADER_SELECTED_REVIEW_STATUS"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "selected_leader_core_pool_count={}",
            read_optional_env_value(selected_env, &["COPYTRADER_CORE_POOL_COUNT"])
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("leader_position_count={}", positions.len()),
        format!("leader_spot_value_usdc={}", leader_state.value.spot_value),
        format!("leader_ewma_value_usdc={}", leader_state.value.ewma_value),
        format!("target_count={}", output.targets.len()),
        format!("delta_count={}", output.deltas.len()),
        format!(
            "diagnostic_stale_asset_count={}",
            output.diagnostics.stale_assets.len()
        ),
        format!(
            "diagnostic_blocked_asset_count={}",
            output.diagnostics.blocked_assets.len()
        ),
        format!(
            "diagnostic_total_target_risk_usdc={}",
            output.diagnostics.total_target_risk_usdc
        ),
        format!("diagnostic_blocker_summary={}", blocker_summary),
    ];
    for target in output.targets.iter().take(10) {
        lines.push(format!(
            "target asset={} risk_usdc={} confidence_bps={} stale={}",
            target.asset.0, target.signed_target_risk_usdc, target.confidence_bps, target.stale
        ));
    }
    fs::write(&report_path, lines.join("\n"))
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;
    Ok(report_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("run-position-targeting-demo-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_demo_flags() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--equity-usdc".into(),
            "2000000000".into(),
            "--ewma-beta-bps".into(),
            "3000".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert_eq!(options.equity_usdc, 2_000_000_000);
        assert_eq!(options.ewma_beta_bps, 3_000);
    }

    #[test]
    fn demo_uses_cached_positions_and_emits_targets() {
        let root = unique_temp_root("demo");
        let discovery = root.join(".omx/discovery");
        let markets = discovery.join("markets");
        fs::create_dir_all(&markets).expect("markets dir created");
        fs::write(
            discovery.join("selected-leader.env"),
            concat!(
                "COPYTRADER_DISCOVERY_WALLET=0xleader\n",
                "COPYTRADER_SELECTED_RANK=1\n",
                "COPYTRADER_SELECTED_REVIEW_STATUS=stable\n",
                "COPYTRADER_CORE_POOL_COUNT=3\n",
            ),
        )
        .expect("selected env written");
        fs::write(
            discovery.join("positions-0xleader.json"),
            "[{\"asset\":\"asset-1\",\"conditionId\":\"cond-1\",\"size\":100.0,\"avgPrice\":0.5,\"initialValue\":50,\"currentValue\":55,\"slug\":\"market-1\",\"eventId\":\"event-1\",\"outcome\":\"Yes\",\"endDate\":\"2026-12-31\",\"negativeRisk\":false}]",
        )
        .expect("positions written");
        fs::write(
            discovery.join("value-0xleader.json"),
            "[{\"user\":\"0xleader\",\"value\":55}]",
        )
        .expect("value written");
        fs::write(
            markets.join("market-1.json"),
            "{\"slug\":\"market-1\",\"conditionId\":\"cond-1\",\"acceptingOrders\":true,\"enableOrderBook\":true,\"liquidityClob\":100000000000,\"volumeClob\":100000000000,\"negRisk\":false}",
        )
        .expect("market written");

        let lines = run_demo(&Options {
            root: root.display().to_string(),
            discovery_dir: None,
            selected_leader_env: None,
            equity_usdc: 1_000_000_000,
            ewma_beta_bps: 2_500,
        })
        .expect("demo should succeed");

        let joined = lines.join("\n");
        assert!(joined.contains("mode=position-targeting-demo"));
        assert!(joined.contains("selected_leader_wallet=0xleader"));
        assert!(joined.contains("target_count=1"));
        assert!(joined.contains("delta_count=1"));
        assert!(joined.contains("diagnostic_blocker_summary=none_detected"));
        assert!(joined.contains("report_path="));

        let report_path = joined
            .lines()
            .find_map(|line| line.strip_prefix("report_path="))
            .expect("report path");
        let report = fs::read_to_string(report_path).expect("report exists");
        assert!(report.contains("leader_ewma_value_usdc=55000000"));
        assert!(report.contains("selected_leader_review_status=stable"));
        assert!(report.contains("diagnostic_blocker_summary=none_detected"));

        fs::remove_dir_all(root).expect("temp root removed");
    }
}
