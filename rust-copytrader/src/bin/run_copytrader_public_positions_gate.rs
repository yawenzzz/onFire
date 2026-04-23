use polymarket_client_sdk::data::Client as SdkDataClient;
use polymarket_client_sdk::data::types::request::PositionsRequest;
use polymarket_client_sdk::data::types::response::Position as DataPosition;
use polymarket_client_sdk::types::{Address as SdkAddress, Decimal as SdkDecimal, U256};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr as _;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    user: String,
    latest_activity: String,
    positions_limit: i32,
    positions_retry_count: u32,
    positions_retry_delay_ms: u64,
    size_epsilon_millis: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            user: String::new(),
            latest_activity: String::new(),
            positions_limit: 500,
            positions_retry_count: 4,
            positions_retry_delay_ms: 750,
            size_epsilon_millis: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LatestActivity {
    tx: String,
    activity_type: String,
    side: String,
    asset: String,
    size: f64,
    price: Option<f64>,
    condition_id: Option<String>,
    slug: Option<String>,
    event_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PositionSummary {
    response_count: usize,
    matched_count: usize,
    target_asset_size: f64,
    other_asset_size: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GateOutcome {
    status: &'static str,
    reason: &'static str,
    should_follow: bool,
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

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to build tokio runtime: {error}");
            return ExitCode::from(1);
        }
    };

    match runtime.block_on(run(&options)) {
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
        "usage: run_copytrader_public_positions_gate --user <wallet> --latest-activity <path> [--positions-limit <n>] [--positions-retry-count <n>] [--positions-retry-delay-ms <n>] [--size-epsilon-millis <n>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--user" => options.user = next_value(&mut iter, arg)?,
            "--latest-activity" => options.latest_activity = next_value(&mut iter, arg)?,
            "--positions-limit" => {
                options.positions_limit =
                    parse_i32(&next_value(&mut iter, arg)?, "positions-limit")?
            }
            "--positions-retry-count" => {
                options.positions_retry_count =
                    parse_u32(&next_value(&mut iter, arg)?, "positions-retry-count")?
            }
            "--positions-retry-delay-ms" => {
                options.positions_retry_delay_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "positions-retry-delay-ms")?
            }
            "--size-epsilon-millis" => {
                options.size_epsilon_millis =
                    parse_u64(&next_value(&mut iter, arg)?, "size-epsilon-millis")?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if options.user.trim().is_empty() {
        return Err("missing --user <wallet>".to_string());
    }
    if options.latest_activity.trim().is_empty() {
        return Err("missing --latest-activity <path>".to_string());
    }
    if options.positions_limit < 0 || options.positions_limit > 500 {
        return Err("positions-limit must be between 0 and 500".to_string());
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

fn parse_i32(value: &str, field: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_u32(value: &str, field: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

fn parse_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid integer for {field}: {value}"))
}

async fn run(options: &Options) -> Result<Vec<String>, String> {
    let latest = read_latest_activity(Path::new(&options.latest_activity))?;
    let epsilon = options.size_epsilon_millis as f64 / 1000.0;
    let initial_outcome = decide_gate(&latest, None, epsilon);
    if initial_outcome.status != "needs_current_positions" {
        return Ok(render_lines(
            options,
            &latest,
            None,
            0,
            &initial_outcome,
            "skipped_without_positions_query",
        ));
    }

    let user = SdkAddress::from_str(&options.user)
        .map_err(|error| format!("invalid user wallet {}: {error}", options.user))?;
    let client = SdkDataClient::default();
    let mut last_summary = None;
    let mut last_outcome = None;
    let mut attempts_used = 0_u32;

    for attempt in 1..=options.positions_retry_count.max(1) {
        attempts_used = attempt;
        let positions = fetch_positions(&client, user, options).await?;
        let summary = summarize_positions(&positions, &latest)?;
        let outcome = decide_gate(&latest, Some(&summary), epsilon);
        let should_retry = outcome.status == "skip_positions_unconfirmed"
            && attempt < options.positions_retry_count.max(1);

        last_summary = Some(summary);
        last_outcome = Some(outcome);

        if !should_retry {
            break;
        }
        sleep(Duration::from_millis(options.positions_retry_delay_ms)).await;
    }

    let summary = last_summary;
    let outcome = last_outcome.unwrap_or_else(|| decide_gate(&latest, None, epsilon));
    Ok(render_lines(
        options,
        &latest,
        summary.as_ref(),
        attempts_used,
        &outcome,
        "ok",
    ))
}

async fn fetch_positions(
    client: &SdkDataClient,
    user: SdkAddress,
    options: &Options,
) -> Result<Vec<DataPosition>, String> {
    let page_limit = options.positions_limit.max(1);
    let mut offset = 0_i32;
    let mut positions = Vec::new();

    loop {
        let builder = PositionsRequest::builder()
            .user(user)
            .size_threshold(SdkDecimal::ZERO);
        let builder = builder
            .limit(page_limit)
            .map_err(|error| format!("invalid positions-limit: {error}"))?;
        let builder = builder
            .offset(offset)
            .map_err(|error| format!("invalid positions offset {offset}: {error}"))?;
        let request = builder.build();
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

fn render_lines(
    options: &Options,
    latest: &LatestActivity,
    summary: Option<&PositionSummary>,
    attempts: u32,
    outcome: &GateOutcome,
    query_status: &str,
) -> Vec<String> {
    let summary = summary.cloned().unwrap_or(PositionSummary {
        response_count: 0,
        matched_count: 0,
        target_asset_size: 0.0,
        other_asset_size: 0.0,
    });
    vec![
        "mode=public-positions-gate".to_string(),
        format!("positions_query_status={query_status}"),
        format!("positions_retry_attempts={attempts}"),
        format!("positions_limit={}", options.positions_limit),
        format!("latest_activity_path={}", options.latest_activity),
        format!("latest_activity_tx={}", latest.tx),
        format!("latest_activity_type={}", latest.activity_type),
        format!("latest_activity_side={}", latest.side),
        format!("latest_activity_asset={}", latest.asset),
        format!("latest_activity_size={:.6}", latest.size),
        format!(
            "latest_activity_condition_id={}",
            latest.condition_id.as_deref().unwrap_or("")
        ),
        format!(
            "latest_activity_slug={}",
            latest.slug.as_deref().unwrap_or("")
        ),
        format!(
            "latest_activity_event_slug={}",
            latest.event_slug.as_deref().unwrap_or("")
        ),
        format!(
            "latest_activity_price={}",
            latest
                .price
                .map(|value| format!("{value:.8}"))
                .unwrap_or_default()
        ),
        format!(
            "current_positions_response_count={}",
            summary.response_count
        ),
        format!("current_event_position_count={}", summary.matched_count),
        format!(
            "current_event_target_asset_size={:.6}",
            summary.target_asset_size
        ),
        format!(
            "current_event_other_asset_size={:.6}",
            summary.other_asset_size
        ),
        format!(
            "current_event_total_size={:.6}",
            summary.target_asset_size + summary.other_asset_size
        ),
        format!("leader_event_open_gate_status={}", outcome.status),
        format!("leader_event_open_gate_reason={}", outcome.reason),
        format!("leader_event_should_follow={}", outcome.should_follow),
    ]
}

fn decide_gate(
    latest: &LatestActivity,
    summary: Option<&PositionSummary>,
    epsilon: f64,
) -> GateOutcome {
    if !latest.activity_type.eq_ignore_ascii_case("TRADE") {
        return GateOutcome {
            status: "skip_non_trade_activity",
            reason: "latest_activity_type_not_trade",
            should_follow: false,
        };
    }
    if !latest.side.eq_ignore_ascii_case("BUY") {
        return GateOutcome {
            status: "skip_non_buy_trade",
            reason: "latest_trade_side_not_buy",
            should_follow: false,
        };
    }
    if latest.condition_id.is_none() && latest.slug.is_none() && latest.event_slug.is_none() {
        return GateOutcome {
            status: "skip_positions_unconfirmed",
            reason: "missing_market_identity_on_latest_trade",
            should_follow: false,
        };
    }

    let Some(summary) = summary else {
        return GateOutcome {
            status: "needs_current_positions",
            reason: "current_positions_required",
            should_follow: false,
        };
    };

    if summary.other_asset_size > epsilon {
        return GateOutcome {
            status: "skip_existing_event_position",
            reason: "wallet_already_holds_other_outcome_in_event",
            should_follow: false,
        };
    }

    if summary.target_asset_size > latest.size + epsilon {
        return GateOutcome {
            status: "skip_existing_event_position",
            reason: "wallet_target_outcome_position_exceeds_latest_trade_size",
            should_follow: false,
        };
    }

    if (summary.target_asset_size - latest.size).abs() <= epsilon {
        return GateOutcome {
            status: "follow_new_open",
            reason: "current_position_matches_latest_trade_size",
            should_follow: true,
        };
    }

    GateOutcome {
        status: "skip_positions_unconfirmed",
        reason: "current_positions_do_not_confirm_new_open",
        should_follow: false,
    }
}

fn summarize_positions(
    positions: &[DataPosition],
    latest: &LatestActivity,
) -> Result<PositionSummary, String> {
    let latest_asset = U256::from_str(&latest.asset).ok();
    let mut matched_count = 0usize;
    let mut target_asset_size = 0.0_f64;
    let mut other_asset_size = 0.0_f64;

    for position in positions {
        if !position_matches_latest(position, latest) {
            continue;
        }
        matched_count += 1;
        let size = position
            .size
            .to_string()
            .parse::<f64>()
            .map_err(|error| format!("invalid position size {}: {error}", position.size))?;
        if latest_asset.is_some_and(|asset| position.asset == asset) {
            target_asset_size += size.abs();
        } else if position.asset.to_string() == latest.asset {
            target_asset_size += size.abs();
        } else {
            other_asset_size += size.abs();
        }
    }

    Ok(PositionSummary {
        response_count: positions.len(),
        matched_count,
        target_asset_size,
        other_asset_size,
    })
}

fn position_matches_latest(position: &DataPosition, latest: &LatestActivity) -> bool {
    if let Some(condition_id) = latest.condition_id.as_deref() {
        return position
            .condition_id
            .to_string()
            .eq_ignore_ascii_case(condition_id);
    }
    if let Some(event_slug) = latest.event_slug.as_deref()
        && position.event_slug.eq_ignore_ascii_case(event_slug)
    {
        return true;
    }
    if let Some(slug) = latest.slug.as_deref()
        && (position.slug.eq_ignore_ascii_case(slug)
            || position.event_slug.eq_ignore_ascii_case(slug))
    {
        return true;
    }
    false
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
        tx: extract_field_value(&object, "transactionHash")
            .ok_or_else(|| "missing transactionHash in latest activity".to_string())?,
        activity_type: extract_field_value(&object, "type").unwrap_or_else(|| "TRADE".to_string()),
        side: extract_field_value(&object, "side").unwrap_or_default(),
        asset: extract_field_value(&object, "asset")
            .ok_or_else(|| "missing asset in latest activity".to_string())?,
        size: extract_field_value(&object, "size")
            .ok_or_else(|| "missing size in latest activity".to_string())?
            .parse::<f64>()
            .map(|value| value.abs())
            .map_err(|error| format!("invalid latest activity size: {error}"))?,
        price: extract_field_value(&object, "price")
            .as_deref()
            .map(parse_decimal_value)
            .transpose()?,
        condition_id: extract_field_value(&object, "conditionId"),
        slug: extract_field_value(&object, "slug"),
        event_slug: extract_field_value(&object, "eventSlug")
            .or_else(|| extract_field_value(&object, "event_slug")),
    })
}

fn parse_decimal_value(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid decimal value: {value}"))
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

#[cfg(test)]
mod tests {
    use super::{GateOutcome, LatestActivity, PositionSummary, decide_gate};

    fn sample_latest() -> LatestActivity {
        LatestActivity {
            tx: "0xabc".into(),
            activity_type: "TRADE".into(),
            side: "BUY".into(),
            asset: "asset-1".into(),
            size: 60.0,
            price: Some(0.5),
            condition_id: Some(
                "0x1111111111111111111111111111111111111111111111111111111111111111".into(),
            ),
            slug: Some("market-a".into()),
            event_slug: Some("event-a".into()),
        }
    }

    fn outcome_tuple(outcome: GateOutcome) -> (&'static str, &'static str, bool) {
        (outcome.status, outcome.reason, outcome.should_follow)
    }

    #[test]
    fn decide_gate_skips_non_buy_trade() {
        let mut latest = sample_latest();
        latest.side = "SELL".into();
        assert_eq!(
            outcome_tuple(decide_gate(&latest, None, 0.001)),
            ("skip_non_buy_trade", "latest_trade_side_not_buy", false,)
        );
    }

    #[test]
    fn decide_gate_allows_new_open_when_position_matches_trade_size() {
        let latest = sample_latest();
        let summary = PositionSummary {
            response_count: 1,
            matched_count: 1,
            target_asset_size: 60.0,
            other_asset_size: 0.0,
        };
        assert_eq!(
            outcome_tuple(decide_gate(&latest, Some(&summary), 0.001)),
            (
                "follow_new_open",
                "current_position_matches_latest_trade_size",
                true,
            )
        );
    }

    #[test]
    fn decide_gate_skips_existing_event_when_other_outcome_present() {
        let latest = sample_latest();
        let summary = PositionSummary {
            response_count: 2,
            matched_count: 2,
            target_asset_size: 60.0,
            other_asset_size: 10.0,
        };
        assert_eq!(
            outcome_tuple(decide_gate(&latest, Some(&summary), 0.001)),
            (
                "skip_existing_event_position",
                "wallet_already_holds_other_outcome_in_event",
                false,
            )
        );
    }

    #[test]
    fn decide_gate_skips_existing_event_when_target_size_exceeds_trade() {
        let latest = sample_latest();
        let summary = PositionSummary {
            response_count: 1,
            matched_count: 1,
            target_asset_size: 90.0,
            other_asset_size: 0.0,
        };
        assert_eq!(
            outcome_tuple(decide_gate(&latest, Some(&summary), 0.001)),
            (
                "skip_existing_event_position",
                "wallet_target_outcome_position_exceeds_latest_trade_size",
                false,
            )
        );
    }

    #[test]
    fn decide_gate_skips_when_positions_do_not_confirm_new_open() {
        let latest = sample_latest();
        let summary = PositionSummary {
            response_count: 1,
            matched_count: 1,
            target_asset_size: 30.0,
            other_asset_size: 0.0,
        };
        assert_eq!(
            outcome_tuple(decide_gate(&latest, Some(&summary), 0.001)),
            (
                "skip_positions_unconfirmed",
                "current_positions_do_not_confirm_new_open",
                false,
            )
        );
    }
}
