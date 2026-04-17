use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ANSI_CLEAR: &str = "\x1b[2J\x1b[H";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_MAGENTA: &str = "\x1b[35m";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: String,
    interval_ms: u64,
    once: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: "..".to_string(),
            interval_ms: 1000,
            once: false,
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

    if options.once {
        match render_dashboard(Path::new(&options.root)) {
            Ok(frame) => {
                print!("{frame}");
                std::process::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::ExitCode::from(1)
            }
        }
    } else {
        loop {
            match render_dashboard(Path::new(&options.root)) {
                Ok(frame) => print!("{frame}"),
                Err(error) => {
                    eprintln!("{error}");
                    return std::process::ExitCode::from(1);
                }
            }
            thread::sleep(Duration::from_millis(options.interval_ms.max(100)));
        }
    }
}

fn print_usage() {
    println!("usage: run_copytrader_ansi_dashboard [--root <path>] [--interval-ms <n>] [--once]");
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => options.root = next_value(&mut iter, arg)?,
            "--interval-ms" => {
                options.interval_ms = parse_u64(&next_value(&mut iter, arg)?, "interval-ms")?
            }
            "--once" => options.once = true,
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

fn render_dashboard(root: &Path) -> Result<String, String> {
    let now = unix_now_secs()?;
    let summary = load_key_values(&Some(
        root.join(".omx/discovery/wallet-filter-v1-summary.txt"),
    ));
    let selected = load_key_values(&Some(root.join(".omx/discovery/selected-leader.env")));
    let operator = load_key_values(&pick_operator_report(root));
    let position = load_key_values(&pick_latest_matching(
        &root.join(".omx/position-targeting"),
        "position-targeting-",
        ".txt",
    ));
    let auto_guarded = load_key_values(&pick_latest_matching(
        &root.join(".omx/auto-guarded"),
        "auto-guarded-",
        ".txt",
    ));

    let mut lines = vec![
        ANSI_CLEAR.to_string(),
        format!(
            "{}{}copytrader ansi dashboard{} {}root={}  now={}{}",
            ANSI_BOLD,
            ANSI_CYAN,
            ANSI_RESET,
            ANSI_DIM,
            root.display(),
            now,
            ANSI_RESET
        ),
        String::new(),
        section(
            "smart-money summary",
            &[
                kv(
                    "best_rejected",
                    color_reject(
                        summary
                            .get("best_rejected_category")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "best_watchlist",
                    color_watch(
                        summary
                            .get("best_watchlist_category")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "watchlist_candidates",
                    plain(
                        summary
                            .get("watchlist_candidates")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "closest_rejected",
                    plain(
                        summary
                            .get("closest_rejected_categories")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
            ],
        ),
        section(
            "selected leader",
            &[
                kv(
                    "wallet",
                    plain(selected_value(
                        &selected,
                        &["COPYTRADER_DISCOVERY_WALLET", "COPYTRADER_LEADER_WALLET"],
                    )),
                ),
                kv(
                    "category",
                    plain(
                        selected
                            .get("COPYTRADER_SELECTED_CATEGORY")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "score",
                    plain(
                        selected
                            .get("COPYTRADER_SELECTED_SCORE")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "review",
                    color_status(
                        selected
                            .get("COPYTRADER_SELECTED_REVIEW_STATUS")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "core_pool",
                    plain(
                        selected
                            .get("COPYTRADER_CORE_POOL_WALLETS")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "active_pool",
                    plain(
                        selected
                            .get("COPYTRADER_ACTIVE_POOL_WALLETS")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
            ],
        ),
        section(
            "operator lane",
            &[
                kv(
                    "last_submit",
                    color_status(
                        operator
                            .get("last_submit_status")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "runtime_review",
                    color_status(
                        operator
                            .get("runtime_subject_review_status")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "core_pool_count",
                    plain(
                        operator
                            .get("runtime_subject_core_pool_count")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "active_pool_count",
                    plain(
                        operator
                            .get("runtime_subject_active_pool_count")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "latency",
                    plain(&format!(
                        "submit={}ms verified={}ms total={}ms",
                        operator
                            .get("replay_submit_elapsed_ms")
                            .map(String::as_str)
                            .unwrap_or("none"),
                        operator
                            .get("replay_verified_elapsed_ms")
                            .map(String::as_str)
                            .unwrap_or("none"),
                        operator
                            .get("last_total_elapsed_ms")
                            .map(String::as_str)
                            .unwrap_or("none")
                    )),
                ),
            ],
        ),
        section(
            "position targeting",
            &[
                kv(
                    "target_count",
                    plain(
                        position
                            .get("target_count")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "delta_count",
                    plain(
                        position
                            .get("delta_count")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "leader_spot",
                    plain(
                        position
                            .get("leader_spot_value_usdc")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "leader_ewma",
                    plain(
                        position
                            .get("leader_ewma_value_usdc")
                            .map(String::as_str)
                            .unwrap_or("none"),
                    ),
                ),
                kv(
                    "first_delta",
                    plain(&format!(
                        "asset={} delta={} tte={}",
                        position
                            .get("delta[0].asset")
                            .map(String::as_str)
                            .unwrap_or("none"),
                        position
                            .get("delta[0].delta_risk_usdc")
                            .map(String::as_str)
                            .unwrap_or("none"),
                        position
                            .get("delta[0].tte_bucket")
                            .map(String::as_str)
                            .unwrap_or("none")
                    )),
                ),
            ],
        ),
        section(
            "auto-guarded",
            &[kv(
                "latest",
                plain(
                    auto_guarded
                        .get("auto_guarded_report_path")
                        .map(String::as_str)
                        .unwrap_or("none"),
                ),
            )],
        ),
    ];
    lines.push(String::new());
    lines.push(format!("{}ctrl+c to stop{}", ANSI_DIM, ANSI_RESET));
    Ok(lines.join("\n"))
}

fn section(title: &str, rows: &[String]) -> String {
    let mut out = vec![format!("{}{}{}", ANSI_BOLD, title, ANSI_RESET)];
    out.extend(rows.iter().cloned());
    out.join("\n")
}

fn kv(label: &str, value: String) -> String {
    format!("  {}{:16}{} {}", ANSI_DIM, label, ANSI_RESET, value)
}

fn plain(value: &str) -> String {
    value.to_string()
}

fn color_status(value: &str) -> String {
    match value {
        "stable" | "verified" | "processed" => format!("{}{}{}", ANSI_GREEN, value, ANSI_RESET),
        "downgrade" | "preview_only" => format!("{}{}{}", ANSI_YELLOW, value, ANSI_RESET),
        "blacklist" | "rejected" | "none" => format!("{}{}{}", ANSI_RED, value, ANSI_RESET),
        other => other.to_string(),
    }
}

fn color_reject(value: &str) -> String {
    if value == "none" {
        format!("{}{}{}", ANSI_DIM, value, ANSI_RESET)
    } else {
        format!("{}{}{}", ANSI_RED, value, ANSI_RESET)
    }
}

fn color_watch(value: &str) -> String {
    if value == "none" {
        format!("{}{}{}", ANSI_DIM, value, ANSI_RESET)
    } else {
        format!("{}{}{}", ANSI_MAGENTA, value, ANSI_RESET)
    }
}

fn selected_value<'a>(map: &'a BTreeMap<String, String>, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| map.get(*key).map(String::as_str))
        .unwrap_or("none")
}

fn load_key_values(path: &Option<PathBuf>) -> BTreeMap<String, String> {
    let Some(path) = path else {
        return BTreeMap::new();
    };
    let Ok(body) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    body.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn pick_operator_report(root: &Path) -> Option<PathBuf> {
    let latest = root.join(".omx/operator-demo/latest.txt");
    if latest.exists() {
        return Some(latest);
    }
    pick_latest_matching(
        &root.join(".omx/operator-demo"),
        "discover-and-demo-",
        ".txt",
    )
}

fn pick_latest_matching(dir: &Path, prefix: &str, suffix: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with(prefix) && name.ends_with(suffix)).then_some(path)
        })
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn unix_now_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_args, render_dashboard};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("ansi-dashboard-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_once_and_interval() {
        let options = parse_args(&[
            "--root".into(),
            "..".into(),
            "--interval-ms".into(),
            "250".into(),
            "--once".into(),
        ])
        .expect("parse");

        assert_eq!(options.root, "..");
        assert_eq!(options.interval_ms, 250);
        assert!(options.once);
    }

    #[test]
    fn render_dashboard_includes_sections_from_artifacts() {
        let root = unique_temp_root("render");
        fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir created");
        fs::create_dir_all(root.join(".omx/operator-demo")).expect("operator demo dir created");
        fs::create_dir_all(root.join(".omx/position-targeting")).expect("position dir created");
        fs::create_dir_all(root.join(".omx/auto-guarded")).expect("auto dir created");

        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            concat!(
                "COPYTRADER_DISCOVERY_WALLET=0xleader\n",
                "COPYTRADER_SELECTED_CATEGORY=TECH\n",
                "COPYTRADER_SELECTED_SCORE=84\n",
                "COPYTRADER_SELECTED_REVIEW_STATUS=stable\n",
                "COPYTRADER_CORE_POOL_WALLETS=0xaaa:95,0xbbb:88\n",
                "COPYTRADER_ACTIVE_POOL_WALLETS=0xaaa:95\n",
            ),
        )
        .expect("selected env written");
        fs::write(
            root.join(".omx/discovery/wallet-filter-v1-summary.txt"),
            concat!(
                "best_rejected_category=TECH\n",
                "best_watchlist_category=TECH\n",
                "watchlist_candidates=TECH:stable:84,POLITICS:downgrade:59\n",
                "closest_rejected_categories=TECH:1:84,FINANCE:2:85\n",
            ),
        )
        .expect("summary written");
        fs::write(
            root.join(".omx/operator-demo/latest.txt"),
            concat!(
                "last_submit_status=verified\n",
                "runtime_subject_review_status=stable\n",
                "runtime_subject_core_pool_count=3\n",
                "runtime_subject_active_pool_count=2\n",
                "replay_submit_elapsed_ms=60\n",
                "replay_verified_elapsed_ms=82\n",
                "last_total_elapsed_ms=82\n",
            ),
        )
        .expect("operator latest written");
        fs::write(
            root.join(".omx/position-targeting/position-targeting-1.txt"),
            concat!(
                "target_count=2\n",
                "delta_count=1\n",
                "leader_spot_value_usdc=55000000\n",
                "leader_ewma_value_usdc=55000000\n",
                "delta[0].asset=asset-1\n",
                "delta[0].delta_risk_usdc=20000000\n",
                "delta[0].tte_bucket=Over72h\n",
            ),
        )
        .expect("position report written");
        fs::write(
            root.join(".omx/auto-guarded/auto-guarded-1.txt"),
            "auto_guarded_report_path=/tmp/demo.txt\n",
        )
        .expect("auto report written");

        let rendered = render_dashboard(&root).expect("dashboard should render");
        assert!(rendered.contains("copytrader ansi dashboard"));
        assert!(rendered.contains("smart-money summary"));
        assert!(rendered.contains("selected leader"));
        assert!(rendered.contains("position targeting"));
        assert!(rendered.contains("watchlist_candidates"));
        assert!(rendered.contains("target_count"));
        assert!(rendered.contains("\x1b[2J\x1b[H"));

        fs::remove_dir_all(root).expect("temp root removed");
    }
}
