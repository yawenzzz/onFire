use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOOKBACK_SECS: u64 = 90 * 24 * 60 * 60;
pub const MIN_COPYABLE_LIQUIDITY_CLOB: f64 = 50_000.0;
pub const MIN_COPYABLE_VOLUME_24H_CLOB: f64 = 20_000.0;
pub const DEFAULT_SPECIALIST_CATEGORIES: [&str; 9] = [
    "POLITICS",
    "SPORTS",
    "CRYPTO",
    "CULTURE",
    "MENTIONS",
    "WEATHER",
    "ECONOMICS",
    "TECH",
    "FINANCE",
];

#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    pub category: String,
    pub time_period: String,
    pub order_by: String,
    pub rank: Option<u64>,
    pub wallet: String,
    pub username: Option<String>,
    pub pnl: f64,
    pub vol: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalletCandidateSeed {
    pub category: String,
    pub wallet: String,
    pub username: Option<String>,
    pub week_rank: Option<u64>,
    pub month_rank: Option<u64>,
    pub all_rank: Option<u64>,
    pub month_pnl: f64,
    pub month_vol: f64,
    pub vol_red_flag: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRecord {
    pub wallet: String,
    pub timestamp: u64,
    pub event_type: String,
    pub size: f64,
    pub usdc_size: f64,
    pub transaction_hash: Option<String>,
    pub price: Option<f64>,
    pub asset: String,
    pub side: Option<String>,
    pub condition_id: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionRecord {
    pub wallet: String,
    pub asset: String,
    pub condition_id: Option<String>,
    pub slug: Option<String>,
    pub current_value: f64,
    pub end_date: Option<String>,
    pub negative_risk: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketRecord {
    pub slug: String,
    pub event_id: Option<String>,
    pub condition_id: Option<String>,
    pub category: Option<String>,
    pub end_date: Option<String>,
    pub accepting_orders: bool,
    pub enable_order_book: bool,
    pub liquidity_clob: f64,
    pub volume24hr_clob: f64,
    pub negative_risk: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatestTradeSummary {
    pub timestamp: Option<String>,
    pub side: Option<String>,
    pub slug: Option<String>,
    pub tx: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalletMetrics {
    pub traded_markets: u64,
    pub current_value: f64,
    pub current_value_to_month_vol: f64,
    pub maker_rebate_count: usize,
    pub flip60_ratio: f64,
    pub median_hold_secs: Option<u64>,
    pub p75_hold_secs: Option<u64>,
    pub tail24_ratio: f64,
    pub tail72_ratio: f64,
    pub neg_risk_share: f64,
    pub copyable_ratio: f64,
    pub category_purity: f64,
    pub unique_markets_90d: usize,
    pub total_net_buy_usdc: f64,
    pub latest_trade: LatestTradeSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalletScoreCard {
    pub seed: WalletCandidateSeed,
    pub metrics: WalletMetrics,
    pub score_total: i64,
    pub persistence_score: i64,
    pub hold_score: i64,
    pub non_tail_score: i64,
    pub non_maker_score: i64,
    pub copyable_score: i64,
    pub simplicity_score: i64,
    pub rejection_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalletSelection {
    pub selected: WalletScoreCard,
    pub candidates: Vec<WalletScoreCard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletPoolEntry {
    pub wallet: String,
    pub score_total: i64,
    pub current_value: String,
}

#[derive(Debug, Clone, PartialEq)]
struct HoldLot {
    timestamp: u64,
    remaining_size: f64,
}

pub fn resolve_category_scope(value: &str) -> Vec<String> {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() || normalized == "SPECIALIST" || normalized == "OVERALL" {
        return DEFAULT_SPECIALIST_CATEGORIES
            .iter()
            .map(|value| value.to_string())
            .collect();
    }

    normalized
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn parse_leaderboard_entries(
    content: &str,
    category: &str,
    time_period: &str,
    order_by: &str,
) -> Vec<LeaderboardEntry> {
    json_objects(content)
        .into_iter()
        .filter_map(|object| {
            let wallet = extract_json_field(&object, "proxyWallet")
                .or_else(|| extract_json_field(&object, "wallet"))?;
            Some(LeaderboardEntry {
                category: category.to_string(),
                time_period: time_period.to_string(),
                order_by: order_by.to_string(),
                rank: extract_json_field(&object, "rank").and_then(|value| parse_u64(&value)),
                wallet,
                username: extract_json_field(&object, "userName"),
                pnl: extract_json_field(&object, "pnl")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
                vol: extract_json_field(&object, "vol")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
            })
        })
        .collect()
}

pub fn parse_activity_records(content: &str) -> Vec<ActivityRecord> {
    json_objects(content)
        .into_iter()
        .filter_map(|object| {
            let wallet = extract_json_field(&object, "proxyWallet")
                .or_else(|| extract_json_field(&object, "user"))
                .unwrap_or_default();
            let timestamp =
                extract_json_field(&object, "timestamp").and_then(|value| parse_u64(&value))?;
            let event_type =
                extract_json_field(&object, "type").unwrap_or_else(|| "TRADE".to_string());
            let asset = extract_json_field(&object, "asset").unwrap_or_default();
            Some(ActivityRecord {
                wallet,
                timestamp,
                event_type,
                size: extract_json_field(&object, "size")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
                usdc_size: extract_json_field(&object, "usdcSize")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
                transaction_hash: extract_json_field(&object, "transactionHash"),
                price: extract_json_field(&object, "price").and_then(|value| parse_f64(&value)),
                asset,
                side: extract_json_field(&object, "side"),
                condition_id: extract_json_field(&object, "conditionId"),
                slug: extract_json_field(&object, "slug"),
            })
        })
        .collect()
}

pub fn parse_position_records(content: &str) -> Vec<PositionRecord> {
    json_objects(content)
        .into_iter()
        .map(|object| PositionRecord {
            wallet: extract_json_field(&object, "proxyWallet").unwrap_or_default(),
            asset: extract_json_field(&object, "asset").unwrap_or_default(),
            condition_id: extract_json_field(&object, "conditionId"),
            slug: extract_json_field(&object, "slug"),
            current_value: extract_json_field(&object, "currentValue")
                .and_then(|value| parse_f64(&value))
                .unwrap_or(0.0),
            end_date: extract_json_field(&object, "endDate"),
            negative_risk: extract_json_bool_field(&object, "negativeRisk").unwrap_or(false),
        })
        .collect()
}

pub fn parse_total_value(content: &str) -> Option<f64> {
    let object = json_objects(content).into_iter().next()?;
    extract_json_field(&object, "value").and_then(|value| parse_f64(&value))
}

pub fn parse_traded_count(content: &str) -> Option<u64> {
    if content.trim_start().starts_with('{') {
        extract_json_field(content, "traded").and_then(|value| parse_u64(&value))
    } else {
        let object = json_objects(content).into_iter().next()?;
        extract_json_field(&object, "traded").and_then(|value| parse_u64(&value))
    }
}

pub fn parse_market_record(content: &str, slug_hint: &str) -> Option<MarketRecord> {
    let object = json_objects(content).into_iter().next()?;
    Some(MarketRecord {
        slug: extract_json_field(&object, "slug").unwrap_or_else(|| slug_hint.to_string()),
        event_id: extract_nested_event_field(content, "id"),
        condition_id: extract_json_field(&object, "conditionId"),
        category: extract_json_field(&object, "category")
            .or_else(|| extract_category_from_tags(content)),
        end_date: extract_json_field(&object, "endDate")
            .or_else(|| extract_json_field(&object, "endDateIso")),
        accepting_orders: extract_json_bool_field(&object, "acceptingOrders").unwrap_or(false),
        enable_order_book: extract_json_bool_field(&object, "enableOrderBook").unwrap_or(false),
        liquidity_clob: extract_json_field(&object, "liquidityClob")
            .or_else(|| extract_json_field(&object, "liquidity"))
            .or_else(|| extract_json_field(&object, "volumeClob"))
            .or_else(|| extract_json_field(&object, "volume"))
            .and_then(|value| parse_f64(&value))
            .unwrap_or(0.0),
        volume24hr_clob: extract_json_field(&object, "volume24hrClob")
            .or_else(|| extract_json_field(&object, "volume24hr"))
            .or_else(|| extract_json_field(&object, "volumeClob"))
            .or_else(|| extract_json_field(&object, "volume"))
            .and_then(|value| parse_f64(&value))
            .unwrap_or(0.0),
        negative_risk: extract_json_bool_field(&object, "negRisk"),
    })
}

pub fn enrich_market_record_from_event(market: &MarketRecord, content: &str) -> MarketRecord {
    let mut enriched = market.clone();
    if enriched.category.is_none() {
        enriched.category = extract_category_from_tags(content);
    }
    if enriched.event_id.is_none() {
        enriched.event_id = extract_json_field(content, "id");
    }
    if let Some(market_object) = extract_market_object_from_event(content, &market.slug) {
        if !enriched.accepting_orders {
            enriched.accepting_orders =
                extract_json_bool_field(&market_object, "acceptingOrders").unwrap_or(false);
        }
        if !enriched.enable_order_book {
            enriched.enable_order_book =
                extract_json_bool_field(&market_object, "enableOrderBook").unwrap_or(false);
        }
        if enriched.liquidity_clob <= 0.0 {
            enriched.liquidity_clob = extract_json_field(&market_object, "liquidityClob")
                .or_else(|| extract_json_field(&market_object, "liquidity"))
                .and_then(|value| parse_f64(&value))
                .unwrap_or(0.0);
        }
        if enriched.volume24hr_clob <= 0.0 {
            enriched.volume24hr_clob = extract_json_field(&market_object, "volume24hrClob")
                .or_else(|| extract_json_field(&market_object, "volume24hr"))
                .or_else(|| extract_json_field(&market_object, "volumeClob"))
                .or_else(|| extract_json_field(&market_object, "volume"))
                .and_then(|value| parse_f64(&value))
                .unwrap_or(0.0);
        }
        if enriched.end_date.is_none() {
            enriched.end_date = extract_json_field(&market_object, "endDate")
                .or_else(|| extract_json_field(&market_object, "endDateIso"));
        }
        if enriched.condition_id.is_none() {
            enriched.condition_id = extract_json_field(&market_object, "conditionId");
        }
        if enriched.negative_risk.is_none() {
            enriched.negative_risk = extract_json_bool_field(&market_object, "negRisk");
        }
    }
    enriched
}

pub fn build_candidate_seeds(
    week_entries: &[LeaderboardEntry],
    month_entries: &[LeaderboardEntry],
    all_entries: &[LeaderboardEntry],
    vol_entries: &[LeaderboardEntry],
) -> Vec<WalletCandidateSeed> {
    let week_by_wallet = week_entries
        .iter()
        .map(|entry| (entry.wallet.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let all_by_wallet = all_entries
        .iter()
        .map(|entry| (entry.wallet.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let vol_wallets = vol_entries
        .iter()
        .map(|entry| entry.wallet.as_str())
        .collect::<BTreeSet<_>>();

    let mut seeds = month_entries
        .iter()
        .filter_map(|month| {
            let week = week_by_wallet.get(month.wallet.as_str())?;
            let all = all_by_wallet.get(month.wallet.as_str());
            Some(WalletCandidateSeed {
                category: month.category.clone(),
                wallet: month.wallet.clone(),
                username: month.username.clone().or_else(|| week.username.clone()),
                week_rank: week.rank,
                month_rank: month.rank,
                all_rank: all.and_then(|entry| entry.rank),
                month_pnl: month.pnl,
                month_vol: month.vol,
                vol_red_flag: vol_wallets.contains(month.wallet.as_str()),
            })
        })
        .collect::<Vec<_>>();
    seeds.sort_by(|left, right| {
        left.month_rank
            .unwrap_or(u64::MAX)
            .cmp(&right.month_rank.unwrap_or(u64::MAX))
            .then_with(|| {
                left.week_rank
                    .unwrap_or(u64::MAX)
                    .cmp(&right.week_rank.unwrap_or(u64::MAX))
            })
    });
    seeds
}

pub fn evaluate_candidate(
    seed: &WalletCandidateSeed,
    activities: &[ActivityRecord],
    positions: &[PositionRecord],
    total_value: Option<f64>,
    traded_markets: u64,
    markets: &BTreeMap<String, MarketRecord>,
    now_ts: u64,
) -> WalletScoreCard {
    let cutoff = now_ts.saturating_sub(LOOKBACK_SECS);
    let recent_activities = activities
        .iter()
        .filter(|record| record.timestamp >= cutoff)
        .cloned()
        .collect::<Vec<_>>();

    let latest_trade = recent_activities
        .iter()
        .filter(|record| record.event_type == "TRADE")
        .max_by_key(|record| record.timestamp);
    let maker_rebate_count = recent_activities
        .iter()
        .filter(|record| record.event_type == "MAKER_REBATE")
        .count();
    let trade_events = recent_activities
        .iter()
        .filter(|record| record.event_type == "TRADE")
        .cloned()
        .collect::<Vec<_>>();
    let buy_events = trade_events
        .iter()
        .filter(|record| record.side.as_deref() == Some("BUY"))
        .cloned()
        .collect::<Vec<_>>();

    let hold_metrics = compute_hold_metrics(&trade_events, now_ts);
    let total_current_value = total_value.unwrap_or_else(|| {
        positions
            .iter()
            .map(|position| position.current_value)
            .sum::<f64>()
    });
    let neg_risk_value = positions
        .iter()
        .filter(|position| position.negative_risk)
        .map(|position| position.current_value)
        .sum::<f64>();
    let neg_risk_share = ratio(neg_risk_value, total_current_value);
    let current_value_to_month_vol = ratio(total_current_value, seed.month_vol);

    let mut total_net_buy_usdc = 0.0;
    let mut tail24_usdc = 0.0;
    let mut tail72_usdc = 0.0;
    let mut copyable_usdc = 0.0;
    let mut by_category = BTreeMap::<String, f64>::new();
    let mut unique_markets = BTreeSet::<String>::new();
    for event in &buy_events {
        let buy_usdc = event.usdc_size.max(0.0);
        if buy_usdc <= 0.0 {
            continue;
        }
        total_net_buy_usdc += buy_usdc;
        let market_key = event
            .slug
            .clone()
            .or_else(|| event.condition_id.clone())
            .unwrap_or_else(|| event.asset.clone());
        unique_markets.insert(market_key);

        if let Some(slug) = event.slug.as_ref()
            && let Some(market) = markets.get(slug)
        {
            if market_is_copyable(market, event.timestamp) {
                copyable_usdc += buy_usdc;
            }
            let category = market
                .category
                .clone()
                .unwrap_or_else(|| seed.category.clone());
            *by_category.entry(category).or_insert(0.0) += buy_usdc;
            if let Some(end_date) = market.end_date.as_ref()
                && let Some(end_ts) = parse_iso8601_timestamp(end_date)
            {
                let remaining = end_ts.saturating_sub(event.timestamp);
                if remaining <= 24 * 60 * 60 {
                    tail24_usdc += buy_usdc;
                }
                if remaining <= 72 * 60 * 60 {
                    tail72_usdc += buy_usdc;
                }
            }
        }
    }

    let category_purity = by_category.values().copied().fold(0.0_f64, f64::max);
    let metrics = WalletMetrics {
        traded_markets,
        current_value: total_current_value,
        current_value_to_month_vol,
        maker_rebate_count,
        flip60_ratio: hold_metrics.flip60_ratio,
        median_hold_secs: hold_metrics.median_hold_secs,
        p75_hold_secs: hold_metrics.p75_hold_secs,
        tail24_ratio: ratio(tail24_usdc, total_net_buy_usdc),
        tail72_ratio: ratio(tail72_usdc, total_net_buy_usdc),
        neg_risk_share,
        copyable_ratio: ratio(copyable_usdc, total_net_buy_usdc),
        category_purity: ratio(category_purity, total_net_buy_usdc),
        unique_markets_90d: unique_markets.len(),
        total_net_buy_usdc,
        latest_trade: LatestTradeSummary {
            timestamp: latest_trade.map(|record| record.timestamp.to_string()),
            side: latest_trade.and_then(|record| record.side.clone()),
            slug: latest_trade.and_then(|record| record.slug.clone()),
            tx: latest_trade.and_then(|record| record.transaction_hash.clone()),
        },
    };

    let rejection_reasons = collect_rejection_reasons(seed, &metrics);
    let persistence_score = 20 + i64::from(seed.all_rank.is_some()) * 5;
    let hold_score = i64::from(metrics.median_hold_secs.unwrap_or(0) >= 24 * 60 * 60) * 10
        + i64::from(metrics.p75_hold_secs.unwrap_or(0) >= 72 * 60 * 60) * 10;
    let non_tail_score = score_non_tail(&metrics);
    let non_maker_score = score_non_maker(seed, &metrics);
    let copyable_score = score_copyable(&metrics);
    let simplicity_score = score_simplicity(&metrics);
    let score_total = persistence_score
        + hold_score
        + non_tail_score
        + non_maker_score
        + copyable_score
        + simplicity_score;

    WalletScoreCard {
        seed: seed.clone(),
        metrics,
        score_total,
        persistence_score,
        hold_score,
        non_tail_score,
        non_maker_score,
        copyable_score,
        simplicity_score,
        rejection_reasons,
    }
}

pub fn choose_wallet(candidates: &[WalletScoreCard], index: usize) -> Option<WalletSelection> {
    let mut passed = candidates
        .iter()
        .filter(|candidate| candidate.rejection_reasons.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    passed.sort_by(|left, right| {
        right
            .score_total
            .cmp(&left.score_total)
            .then_with(|| {
                right
                    .seed
                    .month_pnl
                    .partial_cmp(&left.seed.month_pnl)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                left.seed
                    .month_rank
                    .unwrap_or(u64::MAX)
                    .cmp(&right.seed.month_rank.unwrap_or(u64::MAX))
            })
    });
    let selected = passed.get(index)?.clone();

    let mut all = candidates.to_vec();
    all.sort_by(|left, right| {
        right.score_total.cmp(&left.score_total).then_with(|| {
            right
                .seed
                .month_pnl
                .partial_cmp(&left.seed.month_pnl)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    Some(WalletSelection {
        selected,
        candidates: all,
    })
}

pub fn core_pool(selection: &WalletSelection) -> Vec<WalletPoolEntry> {
    passed_candidates(selection)
        .into_iter()
        .take(5)
        .map(pool_entry)
        .collect()
}

pub fn active_pool(selection: &WalletSelection) -> Vec<WalletPoolEntry> {
    passed_candidates(selection)
        .into_iter()
        .filter(|candidate| candidate.metrics.current_value > 0.0)
        .take(2)
        .map(pool_entry)
        .collect()
}

pub fn render_selected_leader_env(selection: &WalletSelection, source: &str) -> String {
    let selected = &selection.selected;
    let metrics = &selected.metrics;
    let core_pool = core_pool(selection);
    let active_pool = active_pool(selection);
    let mut lines = vec![
        format!("COPYTRADER_DISCOVERY_WALLET={}", selected.seed.wallet),
        format!("COPYTRADER_LEADER_WALLET={}", selected.seed.wallet),
        format!("COPYTRADER_SELECTED_FROM={source}"),
        format!("COPYTRADER_SELECTED_CATEGORY={}", selected.seed.category),
        format!("COPYTRADER_SELECTED_SCORE={}", selected.score_total),
        format!("COPYTRADER_CORE_POOL_COUNT={}", core_pool.len()),
        format!(
            "COPYTRADER_CORE_POOL_WALLETS={}",
            render_pool_wallets(&core_pool)
        ),
        format!("COPYTRADER_ACTIVE_POOL_COUNT={}", active_pool.len()),
        format!(
            "COPYTRADER_ACTIVE_POOL_WALLETS={}",
            render_pool_wallets(&active_pool)
        ),
        format!(
            "COPYTRADER_SELECTED_WEEK_RANK={}",
            format_optional_u64(selected.seed.week_rank)
        ),
        format!(
            "COPYTRADER_SELECTED_MONTH_RANK={}",
            format_optional_u64(selected.seed.month_rank)
        ),
        format!(
            "COPYTRADER_SELECTED_ALL_RANK={}",
            format_optional_u64(selected.seed.all_rank)
        ),
        format!(
            "COPYTRADER_SELECTED_RANK={}",
            format_optional_u64(selected.seed.month_rank)
        ),
        format!("COPYTRADER_SELECTED_PNL={:.6}", selected.seed.month_pnl),
        format!(
            "COPYTRADER_SELECTED_MONTH_VOL={:.6}",
            selected.seed.month_vol
        ),
        format!(
            "COPYTRADER_SELECTED_USERNAME={}",
            selected
                .seed
                .username
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
        ),
        format!(
            "COPYTRADER_SELECTED_VOL_RED_FLAG={}",
            selected.seed.vol_red_flag
        ),
        format!(
            "COPYTRADER_FILTER_MAKER_REBATE_COUNT={}",
            metrics.maker_rebate_count
        ),
        format!("COPYTRADER_FILTER_FLIP60={:.6}", metrics.flip60_ratio),
        format!(
            "COPYTRADER_FILTER_MEDIAN_HOLD_HOURS={}",
            format_optional_hours(metrics.median_hold_secs)
        ),
        format!(
            "COPYTRADER_FILTER_P75_HOLD_HOURS={}",
            format_optional_hours(metrics.p75_hold_secs)
        ),
        format!("COPYTRADER_FILTER_TAIL24={:.6}", metrics.tail24_ratio),
        format!("COPYTRADER_FILTER_TAIL72={:.6}", metrics.tail72_ratio),
        format!(
            "COPYTRADER_FILTER_NEG_RISK_SHARE={:.6}",
            metrics.neg_risk_share
        ),
        format!(
            "COPYTRADER_FILTER_COPYABLE_RATIO={:.6}",
            metrics.copyable_ratio
        ),
        format!(
            "COPYTRADER_FILTER_CATEGORY_PURITY={:.6}",
            metrics.category_purity
        ),
        format!(
            "COPYTRADER_FILTER_CURRENT_VALUE={:.6}",
            metrics.current_value
        ),
        format!(
            "COPYTRADER_FILTER_CURRENT_VALUE_TO_MONTH_VOL={:.6}",
            metrics.current_value_to_month_vol
        ),
        format!(
            "COPYTRADER_FILTER_UNIQUE_MARKETS_90D={}",
            metrics.unique_markets_90d
        ),
        format!(
            "COPYTRADER_FILTER_TRADED_MARKETS={}",
            metrics.traded_markets
        ),
        format!(
            "COPYTRADER_LATEST_ACTIVITY_TIMESTAMP={}",
            metrics
                .latest_trade
                .timestamp
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "COPYTRADER_LATEST_ACTIVITY_SIDE={}",
            metrics
                .latest_trade
                .side
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "COPYTRADER_LATEST_ACTIVITY_SLUG={}",
            metrics
                .latest_trade
                .slug
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "COPYTRADER_LATEST_ACTIVITY_TX={}",
            metrics
                .latest_trade
                .tx
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
    ];
    if !selected.rejection_reasons.is_empty() {
        lines.push(format!(
            "COPYTRADER_FILTER_REJECTION_REASONS={}",
            selected.rejection_reasons.join(",")
        ));
    }
    lines.join("\n") + "\n"
}

pub fn render_wallet_filter_report(selection: &WalletSelection, source_label: &str) -> String {
    let core_pool = core_pool(selection);
    let active_pool = active_pool(selection);
    let mut lines = vec![
        "wallet_filter_strategy=wallet_filter_v1".to_string(),
        format!("wallet_filter_source={source_label}"),
        format!("candidate_count={}", selection.candidates.len()),
        format!("selected_wallet={}", selection.selected.seed.wallet),
        format!("selected_category={}", selection.selected.seed.category),
        format!("selected_score={}", selection.selected.score_total),
        format!("core_pool_count={}", core_pool.len()),
        format!("core_pool_wallets={}", render_pool_wallets(&core_pool)),
        format!("active_pool_count={}", active_pool.len()),
        format!("active_pool_wallets={}", render_pool_wallets(&active_pool)),
    ];
    lines.extend(render_candidate_sections(
        &selection.selected.seed.wallet,
        &selection.candidates,
    ));
    lines.join("\n") + "\n"
}

pub fn render_wallet_filter_rejection_report(
    candidates: &[WalletScoreCard],
    source_label: &str,
) -> String {
    let mut lines = vec![
        "wallet_filter_strategy=wallet_filter_v1".to_string(),
        format!("wallet_filter_source={source_label}"),
        format!("candidate_count={}", candidates.len()),
        "selected_wallet=none".to_string(),
        "selected_category=none".to_string(),
        "selected_score=none".to_string(),
    ];
    lines.extend(render_candidate_sections("", candidates));
    lines.join("\n") + "\n"
}

fn render_candidate_sections(selected_wallet: &str, candidates: &[WalletScoreCard]) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        lines.push(format!("== candidate {index} =="));
        lines.push(format!("wallet={}", candidate.seed.wallet));
        lines.push(format!("category={}", candidate.seed.category));
        lines.push(format!(
            "status={}",
            if !selected_wallet.is_empty() && candidate.seed.wallet == selected_wallet {
                "selected"
            } else if candidate.rejection_reasons.is_empty() {
                "passed"
            } else {
                "rejected"
            }
        ));
        lines.push(format!("score_total={}", candidate.score_total));
        lines.push(format!("persistence_score={}", candidate.persistence_score));
        lines.push(format!("hold_score={}", candidate.hold_score));
        lines.push(format!("non_tail_score={}", candidate.non_tail_score));
        lines.push(format!("non_maker_score={}", candidate.non_maker_score));
        lines.push(format!("copyable_score={}", candidate.copyable_score));
        lines.push(format!("simplicity_score={}", candidate.simplicity_score));
        lines.push(format!(
            "week_rank={}",
            format_optional_u64(candidate.seed.week_rank)
        ));
        lines.push(format!(
            "month_rank={}",
            format_optional_u64(candidate.seed.month_rank)
        ));
        lines.push(format!("month_pnl={:.6}", candidate.seed.month_pnl));
        lines.push(format!("month_vol={:.6}", candidate.seed.month_vol));
        lines.push(format!("vol_red_flag={}", candidate.seed.vol_red_flag));
        lines.push(format!(
            "maker_rebate_count={}",
            candidate.metrics.maker_rebate_count
        ));
        lines.push(format!("flip60={:.6}", candidate.metrics.flip60_ratio));
        lines.push(format!(
            "median_hold_hours={}",
            format_optional_hours(candidate.metrics.median_hold_secs)
        ));
        lines.push(format!(
            "p75_hold_hours={}",
            format_optional_hours(candidate.metrics.p75_hold_secs)
        ));
        lines.push(format!("tail24={:.6}", candidate.metrics.tail24_ratio));
        lines.push(format!("tail72={:.6}", candidate.metrics.tail72_ratio));
        lines.push(format!(
            "neg_risk_share={:.6}",
            candidate.metrics.neg_risk_share
        ));
        lines.push(format!(
            "copyable_ratio={:.6}",
            candidate.metrics.copyable_ratio
        ));
        lines.push(format!(
            "category_purity={:.6}",
            candidate.metrics.category_purity
        ));
        lines.push(format!(
            "current_value_to_month_vol={:.6}",
            candidate.metrics.current_value_to_month_vol
        ));
        lines.push(format!(
            "unique_markets_90d={}",
            candidate.metrics.unique_markets_90d
        ));
        lines.push(format!(
            "traded_markets={}",
            candidate.metrics.traded_markets
        ));
        lines.push(format!(
            "latest_activity_timestamp={}",
            candidate
                .metrics
                .latest_trade
                .timestamp
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ));
        if candidate.rejection_reasons.is_empty() {
            lines.push("rejection_reasons=none".to_string());
        } else {
            lines.push(format!(
                "rejection_reasons={}",
                candidate.rejection_reasons.join(",")
            ));
        }
    }
    lines
}

fn passed_candidates(selection: &WalletSelection) -> Vec<&WalletScoreCard> {
    let mut passed = selection
        .candidates
        .iter()
        .filter(|candidate| candidate.rejection_reasons.is_empty())
        .collect::<Vec<_>>();
    passed.sort_by(|left, right| {
        right
            .score_total
            .cmp(&left.score_total)
            .then_with(|| {
                right
                    .metrics
                    .current_value
                    .partial_cmp(&left.metrics.current_value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                right
                    .seed
                    .month_pnl
                    .partial_cmp(&left.seed.month_pnl)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    passed
}

fn pool_entry(candidate: &WalletScoreCard) -> WalletPoolEntry {
    WalletPoolEntry {
        wallet: candidate.seed.wallet.clone(),
        score_total: candidate.score_total,
        current_value: format!("{:.6}", candidate.metrics.current_value),
    }
}

fn render_pool_wallets(pool: &[WalletPoolEntry]) -> String {
    if pool.is_empty() {
        return "none".to_string();
    }
    pool.iter()
        .map(|entry| format!("{}:{}", entry.wallet, entry.score_total))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn json_objects(content: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = None;
    for (idx, byte) in content.as_bytes().iter().enumerate() {
        match byte {
            b'\\' if in_string && !escaped => {
                escaped = true;
                continue;
            }
            b'"' if !escaped => in_string = !in_string,
            b'{' if !in_string => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0
                    && let Some(start) = start.take()
                {
                    objects.push(content[start..=idx].to_string());
                }
            }
            _ => {}
        }
        escaped = false;
    }
    objects
}

fn extract_nested_event_field(content: &str, field: &str) -> Option<String> {
    let anchor = content.find("\"events\":[")?;
    let event_section = &content[anchor..];
    let event_object = json_objects(event_section).into_iter().next()?;
    extract_json_field(&event_object, field)
}

fn extract_market_object_from_event(content: &str, slug: &str) -> Option<String> {
    let anchor = content.find("\"markets\":[")?;
    let section = &content[anchor..];
    json_objects(section)
        .into_iter()
        .find(|object| extract_json_field(object, "slug").as_deref() == Some(slug))
}

fn extract_category_from_tags(content: &str) -> Option<String> {
    let anchor = content.find("\"tags\":[")?;
    let section = &content[anchor..];
    for object in json_objects(section) {
        let slug = extract_json_field(&object, "slug")
            .unwrap_or_default()
            .to_uppercase();
        let label = extract_json_field(&object, "label")
            .unwrap_or_default()
            .to_uppercase();
        for category in DEFAULT_SPECIALIST_CATEGORIES {
            if slug == category || label == category {
                return Some(category.to_string());
            }
        }
    }
    None
}

pub fn extract_json_field(object: &str, field: &str) -> Option<String> {
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

pub fn extract_json_bool_field(object: &str, field: &str) -> Option<bool> {
    extract_json_field(object, field).and_then(|value| match value.trim() {
        "true" | "TRUE" => Some(true),
        "false" | "FALSE" => Some(false),
        _ => None,
    })
}

pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().trim_matches('"').parse::<f64>().ok()
}

pub fn parse_u64(value: &str) -> Option<u64> {
    value.trim().trim_matches('"').parse::<u64>().ok()
}

pub fn parse_iso8601_timestamp(value: &str) -> Option<u64> {
    let value = value.trim();
    let value = value.strip_suffix('Z').unwrap_or(value);
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second_part = time_parts.next()?;
    let second = second_part.split('.').next()?.parse::<u32>().ok()?;
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i64;
    let day = day as i64;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days >= 0).then_some(days as u64)
}

fn market_is_copyable(market: &MarketRecord, observed_at: u64) -> bool {
    let order_window_open = market.accepting_orders
        || market
            .end_date
            .as_deref()
            .and_then(parse_iso8601_timestamp)
            .map(|end_ts| observed_at <= end_ts)
            .unwrap_or(false);
    order_window_open
        && market.enable_order_book
        && market.liquidity_clob >= MIN_COPYABLE_LIQUIDITY_CLOB
        && market.volume24hr_clob >= MIN_COPYABLE_VOLUME_24H_CLOB
}

fn compute_hold_metrics(events: &[ActivityRecord], now_ts: u64) -> HoldMetrics {
    let mut by_asset = HashMap::<String, VecDeque<HoldLot>>::new();
    let mut weighted_durations = Vec::<(u64, f64)>::new();
    let mut matched_sell_size = 0.0;
    let mut quick_flip_size = 0.0;
    let mut ordered = events.to_vec();
    ordered.sort_by_key(|event| event.timestamp);

    for event in &ordered {
        let side = event.side.as_deref().unwrap_or("");
        let size = event.size.max(0.0);
        if size <= 0.0 {
            continue;
        }
        let lots = by_asset.entry(event.asset.clone()).or_default();
        if side == "BUY" {
            lots.push_back(HoldLot {
                timestamp: event.timestamp,
                remaining_size: size,
            });
            continue;
        }
        if side != "SELL" {
            continue;
        }
        let mut remaining = size;
        while remaining > 0.0 {
            let Some(mut lot) = lots.pop_front() else {
                break;
            };
            let matched = remaining.min(lot.remaining_size);
            let duration = event.timestamp.saturating_sub(lot.timestamp);
            weighted_durations.push((duration, matched));
            matched_sell_size += matched;
            if duration <= 60 * 60 {
                quick_flip_size += matched;
            }
            remaining -= matched;
            lot.remaining_size -= matched;
            if lot.remaining_size > 0.0 {
                lots.push_front(lot);
                break;
            }
        }
    }

    for lots in by_asset.values() {
        for lot in lots {
            weighted_durations.push((now_ts.saturating_sub(lot.timestamp), lot.remaining_size));
        }
    }

    HoldMetrics {
        median_hold_secs: weighted_quantile(&weighted_durations, 0.50),
        p75_hold_secs: weighted_quantile(&weighted_durations, 0.75),
        flip60_ratio: ratio(quick_flip_size, matched_sell_size),
    }
}

fn weighted_quantile(values: &[(u64, f64)], quantile: f64) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by_key(|(duration, _)| *duration);
    let total_weight = ordered.iter().map(|(_, weight)| *weight).sum::<f64>();
    if total_weight <= 0.0 {
        return None;
    }
    let threshold = total_weight * quantile.clamp(0.0, 1.0);
    let mut seen = 0.0;
    for (duration, weight) in ordered {
        seen += weight;
        if seen >= threshold {
            return Some(duration);
        }
    }
    values.last().map(|(duration, _)| *duration)
}

fn collect_rejection_reasons(seed: &WalletCandidateSeed, metrics: &WalletMetrics) -> Vec<String> {
    let mut reasons = Vec::new();
    if metrics.total_net_buy_usdc <= 0.0 {
        reasons.push("no_net_buy_activity".to_string());
    }
    if metrics.maker_rebate_count > 0 {
        reasons.push("maker_rebate_detected".to_string());
    }
    if metrics.flip60_ratio > 0.25 {
        reasons.push("flip60_above_25pct".to_string());
    }
    if metrics.median_hold_secs.unwrap_or(0) < 6 * 60 * 60 {
        reasons.push("median_hold_below_6h".to_string());
    }
    if metrics.current_value_to_month_vol < 0.01 && metrics.flip60_ratio > 0.20 {
        reasons.push("low_inventory_high_turnover".to_string());
    }
    if metrics.tail24_ratio > 0.10 {
        reasons.push("tail24_above_10pct".to_string());
    }
    if metrics.tail72_ratio > 0.25 {
        reasons.push("tail72_above_25pct".to_string());
    }
    if metrics.neg_risk_share > 0.20 {
        reasons.push("neg_risk_share_above_20pct".to_string());
    }
    if metrics.copyable_ratio < 0.70 {
        reasons.push("copyable_ratio_below_70pct".to_string());
    }
    if metrics.category_purity < 0.60 {
        reasons.push("category_purity_below_60pct".to_string());
    }
    if metrics.unique_markets_90d < 8 {
        reasons.push("unique_markets_below_8".to_string());
    }
    if metrics.unique_markets_90d > 40 {
        reasons.push("unique_markets_above_40".to_string());
    }
    if metrics.traded_markets < 20 {
        reasons.push("traded_markets_below_20".to_string());
    }
    if seed.category == "OVERALL" {
        reasons.push("overall_category_not_allowed".to_string());
    }
    reasons
}

fn score_non_tail(metrics: &WalletMetrics) -> i64 {
    if metrics.total_net_buy_usdc <= 0.0 {
        return 0;
    }
    let tail24_component = (1.0 - (metrics.tail24_ratio / 0.10)).clamp(0.0, 1.0);
    let tail72_component = (1.0 - (metrics.tail72_ratio / 0.25)).clamp(0.0, 1.0);
    ((tail24_component * 7.5) + (tail72_component * 7.5)).round() as i64
}

fn score_non_maker(seed: &WalletCandidateSeed, metrics: &WalletMetrics) -> i64 {
    if metrics.maker_rebate_count > 0 {
        return 0;
    }
    let flip_score = (1.0 - (metrics.flip60_ratio / 0.25)).clamp(0.0, 1.0) * 10.0;
    let inventory_score = (metrics.current_value_to_month_vol / 0.05).clamp(0.0, 1.0) * 5.0;
    let mut total = (flip_score + inventory_score).round() as i64;
    if seed.vol_red_flag {
        total = total.saturating_sub(3);
    }
    total
}

fn score_copyable(metrics: &WalletMetrics) -> i64 {
    ((metrics.copyable_ratio / 0.80).clamp(0.0, 1.0) * 15.0).round() as i64
}

fn score_simplicity(metrics: &WalletMetrics) -> i64 {
    let neg_risk_component = (1.0 - (metrics.neg_risk_share / 0.10)).clamp(0.0, 1.0) * 4.0;
    let purity_component = (metrics.category_purity / 0.70).clamp(0.0, 1.0) * 4.0;
    let dispersion_component = if (8..=25).contains(&metrics.unique_markets_90d) {
        2.0
    } else if (26..=40).contains(&metrics.unique_markets_90d) {
        1.0
    } else {
        0.0
    };
    (neg_risk_component + purity_component + dispersion_component).round() as i64
}

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator <= 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn format_optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn format_optional_hours(value: Option<u64>) -> String {
    value
        .map(|value| format!("{:.3}", value as f64 / 3600.0))
        .unwrap_or_else(|| "none".to_string())
}

#[derive(Debug, Clone, PartialEq)]
struct HoldMetrics {
    median_hold_secs: Option<u64>,
    p75_hold_secs: Option<u64>,
    flip60_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::{
        ActivityRecord, LatestTradeSummary, MarketRecord, PositionRecord, WalletCandidateSeed,
        active_pool, build_candidate_seeds, choose_wallet, core_pool,
        enrich_market_record_from_event, evaluate_candidate, parse_activity_records,
        parse_iso8601_timestamp, parse_leaderboard_entries, parse_market_record,
        parse_position_records, parse_total_value, parse_traded_count, render_selected_leader_env,
        resolve_category_scope,
    };
    use std::collections::BTreeMap;

    #[test]
    fn specialist_scope_expands_into_non_overall_categories() {
        let categories = resolve_category_scope("SPECIALIST");
        assert!(categories.contains(&"SPORTS".to_string()));
        assert!(!categories.contains(&"OVERALL".to_string()));
    }

    #[test]
    fn leaderboard_intersection_requires_week_and_month_presence() {
        let week = parse_leaderboard_entries(
            r#"[{"rank":"1","proxyWallet":"0xaaa","userName":"alpha","pnl":10,"vol":100},{"rank":"2","proxyWallet":"0xbbb","userName":"beta","pnl":11,"vol":120}]"#,
            "SPORTS",
            "WEEK",
            "PNL",
        );
        let month = parse_leaderboard_entries(
            r#"[{"rank":"3","proxyWallet":"0xbbb","userName":"beta","pnl":50,"vol":400},{"rank":"4","proxyWallet":"0xccc","userName":"gamma","pnl":20,"vol":300}]"#,
            "SPORTS",
            "MONTH",
            "PNL",
        );
        let all = parse_leaderboard_entries(
            r#"[{"rank":"5","proxyWallet":"0xbbb","userName":"beta","pnl":70,"vol":900}]"#,
            "SPORTS",
            "ALL",
            "PNL",
        );
        let vol = parse_leaderboard_entries(
            r#"[{"rank":"1","proxyWallet":"0xbbb","userName":"beta","pnl":70,"vol":900}]"#,
            "SPORTS",
            "MONTH",
            "VOL",
        );

        let seeds = build_candidate_seeds(&week, &month, &all, &vol);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].wallet, "0xbbb");
        assert_eq!(seeds[0].month_rank, Some(3));
        assert_eq!(seeds[0].all_rank, Some(5));
        assert!(seeds[0].vol_red_flag);
    }

    #[test]
    fn parser_extracts_profile_and_market_fields() {
        let activity = parse_activity_records(
            r#"[{"proxyWallet":"0xabc","timestamp":100,"type":"TRADE","size":10,"usdcSize":20,"transactionHash":"0xtx","price":0.5,"asset":"asset-1","side":"BUY","conditionId":"0xcond","slug":"market-a"},{"proxyWallet":"0xabc","timestamp":110,"type":"MAKER_REBATE","size":0,"usdcSize":1,"asset":"asset-1"}]"#,
        );
        let positions = parse_position_records(
            r#"[{"proxyWallet":"0xabc","asset":"asset-1","conditionId":"0xcond","slug":"market-a","currentValue":123,"endDate":"2026-05-01T00:00:00Z","negativeRisk":true}]"#,
        );
        let value = parse_total_value(r#"[{"user":"0xabc","value":333}]"#);
        let traded = parse_traded_count(r#"{"user":"0xabc","traded":22}"#);
        let market = parse_market_record(
            r#"{"slug":"market-a","conditionId":"0xcond","category":"SPORTS","endDate":"2026-05-01T00:00:00Z","acceptingOrders":true,"enableOrderBook":true,"liquidityClob":70000,"volume24hrClob":30000,"negRisk":false}"#,
            "market-a",
        )
        .expect("market");

        assert_eq!(activity.len(), 2);
        assert_eq!(positions.len(), 1);
        assert_eq!(value, Some(333.0));
        assert_eq!(traded, Some(22));
        assert_eq!(market.category.as_deref(), Some("SPORTS"));
        assert!(market.accepting_orders);
    }

    #[test]
    fn event_enrichment_pulls_category_and_volume_fallbacks() {
        let market = parse_market_record(
            r#"{"id":"1606200","slug":"market-a","conditionId":"0xcond","acceptingOrders":false,"enableOrderBook":true,"volumeClob":60000,"events":[{"id":"275896"}]}"#,
            "market-a",
        )
        .expect("market");
        let enriched = enrich_market_record_from_event(
            &market,
            r#"{"id":"275896","tags":[{"label":"Finance","slug":"finance"}],"markets":[{"slug":"market-a","conditionId":"0xcond","acceptingOrders":false,"enableOrderBook":true,"volumeClob":60000,"negRisk":false,"endDate":"2026-12-31T00:00:00Z"}]}"#,
        );

        assert_eq!(enriched.event_id.as_deref(), Some("275896"));
        assert_eq!(enriched.category.as_deref(), Some("FINANCE"));
        assert_eq!(enriched.volume24hr_clob, 60000.0);
    }

    #[test]
    fn candidate_evaluation_rejects_maker_like_wallets() {
        let seed = WalletCandidateSeed {
            category: "SPORTS".into(),
            wallet: "0xwallet".into(),
            username: Some("alpha".into()),
            week_rank: Some(2),
            month_rank: Some(3),
            all_rank: Some(5),
            month_pnl: 100.0,
            month_vol: 2000.0,
            vol_red_flag: false,
        };
        let activities = vec![
            ActivityRecord {
                wallet: "0xwallet".into(),
                timestamp: 1_000,
                event_type: "TRADE".into(),
                size: 10.0,
                usdc_size: 20.0,
                transaction_hash: Some("0x1".into()),
                price: Some(0.5),
                asset: "asset-1".into(),
                side: Some("BUY".into()),
                condition_id: Some("0xcond".into()),
                slug: Some("market-a".into()),
            },
            ActivityRecord {
                wallet: "0xwallet".into(),
                timestamp: 1_100,
                event_type: "MAKER_REBATE".into(),
                size: 0.0,
                usdc_size: 1.0,
                transaction_hash: None,
                price: None,
                asset: "asset-1".into(),
                side: None,
                condition_id: None,
                slug: None,
            },
        ];
        let positions = vec![PositionRecord {
            wallet: "0xwallet".into(),
            asset: "asset-1".into(),
            condition_id: Some("0xcond".into()),
            slug: Some("market-a".into()),
            current_value: 100.0,
            end_date: Some("2026-05-01T00:00:00Z".into()),
            negative_risk: false,
        }];
        let mut markets = BTreeMap::new();
        markets.insert(
            "market-a".into(),
            MarketRecord {
                slug: "market-a".into(),
                event_id: None,
                condition_id: Some("0xcond".into()),
                category: Some("SPORTS".into()),
                end_date: Some("2026-05-01T00:00:00Z".into()),
                accepting_orders: true,
                enable_order_book: true,
                liquidity_clob: 80_000.0,
                volume24hr_clob: 30_000.0,
                negative_risk: Some(false),
            },
        );

        let card = evaluate_candidate(
            &seed,
            &activities,
            &positions,
            Some(100.0),
            25,
            &markets,
            1_200,
        );
        assert!(
            card.rejection_reasons
                .contains(&"maker_rebate_detected".to_string())
        );
    }

    #[test]
    fn choose_wallet_prefers_passed_high_score_candidate_and_renders_env() {
        let seed = WalletCandidateSeed {
            category: "SPORTS".into(),
            wallet: "0xwallet".into(),
            username: Some("alpha".into()),
            week_rank: Some(1),
            month_rank: Some(2),
            all_rank: Some(3),
            month_pnl: 1000.0,
            month_vol: 10_000.0,
            vol_red_flag: false,
        };
        let pass = super::WalletScoreCard {
            seed: seed.clone(),
            metrics: super::WalletMetrics {
                traded_markets: 25,
                current_value: 500.0,
                current_value_to_month_vol: 0.05,
                maker_rebate_count: 0,
                flip60_ratio: 0.01,
                median_hold_secs: Some(48 * 3600),
                p75_hold_secs: Some(96 * 3600),
                tail24_ratio: 0.01,
                tail72_ratio: 0.05,
                neg_risk_share: 0.02,
                copyable_ratio: 0.90,
                category_purity: 0.80,
                unique_markets_90d: 12,
                total_net_buy_usdc: 1000.0,
                latest_trade: LatestTradeSummary {
                    timestamp: Some("123".into()),
                    side: Some("BUY".into()),
                    slug: Some("market-a".into()),
                    tx: Some("0xtx".into()),
                },
            },
            score_total: 95,
            persistence_score: 25,
            hold_score: 20,
            non_tail_score: 15,
            non_maker_score: 15,
            copyable_score: 15,
            simplicity_score: 5,
            rejection_reasons: Vec::new(),
        };
        let fail = super::WalletScoreCard {
            seed: WalletCandidateSeed {
                wallet: "0xfail".into(),
                ..seed
            },
            metrics: pass.metrics.clone(),
            score_total: 10,
            persistence_score: 0,
            hold_score: 0,
            non_tail_score: 0,
            non_maker_score: 0,
            copyable_score: 0,
            simplicity_score: 10,
            rejection_reasons: vec!["maker_rebate_detected".into()],
        };

        let selection = choose_wallet(&[fail, pass.clone()], 0).expect("selection");
        assert_eq!(selection.selected.seed.wallet, "0xwallet");
        let env = render_selected_leader_env(&selection, "wallet_filter_v1");
        let core = core_pool(&selection);
        let active = active_pool(&selection);
        assert!(env.contains("COPYTRADER_SELECTED_CATEGORY=SPORTS"));
        assert!(env.contains("COPYTRADER_SELECTED_SCORE=95"));
        assert!(env.contains("COPYTRADER_CORE_POOL_COUNT=1"));
        assert!(env.contains("COPYTRADER_ACTIVE_POOL_COUNT=1"));
        assert_eq!(core.len(), 1);
        assert_eq!(active.len(), 1);
        assert_eq!(core[0].wallet, "0xwallet");
    }

    #[test]
    fn iso8601_parser_handles_zulu_timestamps() {
        assert_eq!(parse_iso8601_timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_iso8601_timestamp("1970-01-02T00:00:00Z"),
            Some(86_400)
        );
    }
}
