use rust_copytrader::wallet_filter::resolve_category_scope;
use std::cmp::Reverse;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    discovery_dir: String,
    categories: String,
    limit: usize,
    proxy: Option<String>,
    connect_timeout_ms: u64,
    max_time_ms: u64,
    discover_bin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CategoryScanResult {
    category: String,
    status: &'static str,
    selected_wallet: Option<String>,
    selected_score: Option<i64>,
    top_rejected_score: Option<i64>,
    top_rejected_wallet: Option<String>,
    top_rejection_reasons: Option<String>,
    section: String,
}

impl CategoryScanResult {
    fn rejection_reason_count(&self) -> usize {
        self.top_rejection_reasons
            .as_deref()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .count()
            })
            .unwrap_or(0)
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            discovery_dir: "../.omx/discovery".to_string(),
            categories: "SPECIALIST".to_string(),
            limit: 1,
            proxy: env::var("POLYMARKET_CURL_PROXY").ok(),
            connect_timeout_ms: 5_000,
            max_time_ms: 12_000,
            discover_bin: None,
        }
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

    match run_scan(&options) {
        Ok((summary_path, has_pass)) => {
            println!("wallet_filter_summary_path={}", summary_path.display());
            if has_pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!(
        "usage: scan_copy_leader_categories [--discovery-dir <path>] [--categories <SPECIALIST|CSV>] [--limit <n>] [--proxy <url>] [--connect-timeout-ms <n>] [--max-time-ms <n>] [--discover-bin <path>]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--discovery-dir" => options.discovery_dir = next_value(&mut iter, arg)?,
            "--categories" => options.categories = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--proxy" => options.proxy = Some(next_value(&mut iter, arg)?),
            "--connect-timeout-ms" => {
                options.connect_timeout_ms =
                    parse_u64(&next_value(&mut iter, arg)?, "connect-timeout-ms")?
            }
            "--max-time-ms" => {
                options.max_time_ms = parse_u64(&next_value(&mut iter, arg)?, "max-time-ms")?
            }
            "--discover-bin" => options.discover_bin = Some(next_value(&mut iter, arg)?),
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

fn run_scan(options: &Options) -> Result<(PathBuf, bool), String> {
    let discovery_dir = PathBuf::from(&options.discovery_dir);
    fs::create_dir_all(&discovery_dir)
        .map_err(|error| format!("failed to create {}: {error}", discovery_dir.display()))?;
    let summary_path = discovery_dir.join("wallet-filter-v1-summary.txt");
    let discover_bin = resolve_bin_path("discover_copy_leader", options.discover_bin.as_deref())
        .map_err(|error| format!("failed to resolve discover_copy_leader: {error}"))?;
    let categories = resolve_category_scope(&options.categories);
    if categories.is_empty() {
        return Err("no categories resolved for wallet-filter summary scan".to_string());
    }

    let mut sections = Vec::new();
    let mut results = Vec::new();
    let mut has_pass = false;
    for category in categories {
        let output = run_command(
            &discover_bin,
            &build_discover_args(options, &category),
            Some(Path::new(".")),
        );
        let report_path = discovery_dir.join(format!(
            "wallet-filter-v1-{}.txt",
            sanitize_for_filename(&category.to_lowercase())
        ));
        let report = fs::read_to_string(&report_path).ok();
        let section = match output {
            Ok(output) => {
                has_pass = true;
                let stdout = String::from_utf8(output.stdout).map_err(|error| {
                    format!("discover_copy_leader stdout was not utf-8: {error}")
                })?;
                format!(
                    "== category {category} ==\nstatus=passed\n{}{}",
                    stdout.trim_end(),
                    report
                        .as_deref()
                        .map(|body| format!(
                            "\nreport_path={}\n{}",
                            report_path.display(),
                            body.trim_end()
                        ))
                        .unwrap_or_default()
                )
            }
            Err(error) => format!(
                "== category {category} ==\nstatus=rejected\nerror={error}{}",
                report
                    .as_deref()
                    .map(|body| format!(
                        "\nreport_path={}\n{}",
                        report_path.display(),
                        body.trim_end()
                    ))
                    .unwrap_or_default()
            ),
        };
        let selected_wallet = parse_key_from_section(&section, "selected_wallet");
        let selected_score =
            parse_key_from_section(&section, "selected_score").and_then(|value| value.parse().ok());
        let top_rejected_score =
            parse_first_candidate_score(&section).and_then(|value| value.parse().ok());
        let top_rejected_wallet = parse_key_from_error(&section, "top_rejected_wallet=");
        let top_rejection_reasons = parse_key_from_error(&section, "top_rejection_reasons=");
        results.push(CategoryScanResult {
            category: category.clone(),
            status: if section.contains("\nstatus=passed\n") {
                "passed"
            } else {
                "rejected"
            },
            selected_wallet,
            selected_score,
            top_rejected_score,
            top_rejected_wallet,
            top_rejection_reasons,
            section: section.clone(),
        });
        sections.push(section);
    }

    let summary = render_summary(&results);
    fs::write(&summary_path, summary)
        .map_err(|error| format!("failed to write {}: {error}", summary_path.display()))?;
    Ok((summary_path, has_pass))
}

fn render_summary(results: &[CategoryScanResult]) -> String {
    let passed = results
        .iter()
        .filter(|result| result.status == "passed")
        .collect::<Vec<_>>();
    let rejected = results
        .iter()
        .filter(|result| result.status == "rejected")
        .collect::<Vec<_>>();
    let best_pass = passed
        .iter()
        .max_by_key(|result| result.selected_score.unwrap_or(i64::MIN));
    let best_reject = rejected.iter().min_by_key(|result| {
        (
            result.rejection_reason_count(),
            Reverse(result.top_rejected_score.unwrap_or(i64::MIN)),
        )
    });
    let mut rejected_ranked = rejected.to_vec();
    rejected_ranked.sort_by_key(|result| {
        (
            result.rejection_reason_count(),
            Reverse(result.top_rejected_score.unwrap_or(i64::MIN)),
        )
    });
    let closest_rejected = rejected_ranked
        .iter()
        .take(3)
        .map(|result| {
            format!(
                "{}:{}:{}",
                result.category,
                result.rejection_reason_count(),
                result.top_rejected_score.unwrap_or(i64::MIN)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let mut lines = vec![
        "wallet_filter_summary_strategy=wallet_filter_v1".to_string(),
        format!("categories_scanned={}", results.len()),
        format!("categories_passed={}", passed.len()),
        format!("categories_rejected={}", rejected.len()),
        format!(
            "best_pass_category={}",
            best_pass
                .map(|result| result.category.as_str())
                .unwrap_or("none")
        ),
        format!(
            "best_pass_wallet={}",
            best_pass
                .and_then(|result| result.selected_wallet.as_deref())
                .unwrap_or("none")
        ),
        format!(
            "best_rejected_category={}",
            best_reject
                .map(|result| result.category.as_str())
                .unwrap_or("none")
        ),
        format!(
            "best_rejected_wallet={}",
            best_reject
                .and_then(|result| {
                    result
                        .top_rejected_wallet
                        .as_deref()
                        .or(result.selected_wallet.as_deref())
                })
                .unwrap_or("none")
        ),
        format!(
            "best_rejected_reasons={}",
            best_reject
                .and_then(|result| result.top_rejection_reasons.as_deref())
                .unwrap_or("none")
        ),
        format!(
            "best_rejected_reason_count={}",
            best_reject
                .map(|result| result.rejection_reason_count().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "closest_rejected_categories={}",
            if closest_rejected.is_empty() {
                "none".to_string()
            } else {
                closest_rejected
            }
        ),
    ];
    for result in results {
        lines.push(String::new());
        lines.push(result.section.clone());
    }
    lines.join("\n")
}

fn parse_key_from_section(section: &str, key: &str) -> Option<String> {
    section
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "none")
        .map(ToString::to_string)
}

fn parse_key_from_error(section: &str, prefix: &str) -> Option<String> {
    section
        .lines()
        .find_map(|line| line.split(prefix).nth(1))
        .map(|value| {
            value
                .split_whitespace()
                .next()
                .unwrap_or(value)
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn parse_first_candidate_score(section: &str) -> Option<String> {
    let mut in_first_candidate = false;
    for line in section.lines() {
        if line.starts_with("== candidate 0 ==") {
            in_first_candidate = true;
            continue;
        }
        if in_first_candidate {
            if line.starts_with("== candidate ") {
                break;
            }
            if let Some(value) = line.strip_prefix("score_total=") {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn build_discover_args(options: &Options, category: &str) -> Vec<String> {
    let mut args = vec![
        "--discovery-dir".to_string(),
        options.discovery_dir.clone(),
        "--category".to_string(),
        category.to_string(),
        "--limit".to_string(),
        options.limit.to_string(),
        "--connect-timeout-ms".to_string(),
        options.connect_timeout_ms.to_string(),
        "--max-time-ms".to_string(),
        options.max_time_ms.to_string(),
    ];
    if let Some(proxy) = &options.proxy {
        args.push("--proxy".to_string());
        args.push(proxy.clone());
    }
    args
}

fn resolve_bin_path(binary_name: &str, override_path: Option<&str>) -> io::Result<PathBuf> {
    if let Some(override_path) = override_path {
        return Ok(PathBuf::from(override_path));
    }

    let current = env::current_exe()?;
    let current_dir = current
        .parent()
        .ok_or_else(|| io::Error::other("current exe has no parent directory"))?;

    let direct = current_dir.join(binary_name);
    if direct.exists() {
        return Ok(direct);
    }

    if current_dir.ends_with("deps") {
        let sibling = current_dir
            .parent()
            .ok_or_else(|| io::Error::other("deps dir has no parent"))?
            .join(binary_name);
        if sibling.exists() {
            return Ok(sibling);
        }
    }

    Ok(direct)
}

fn run_command(program: &Path, args: &[String], cwd: Option<&Path>) -> Result<Output, String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to execute {}: {error}", program.display()))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{} exited with {}: {} {}",
            program.display(),
            output.status.code().unwrap_or(1),
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        ))
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

#[cfg(test)]
mod tests {
    use super::{build_discover_args, parse_args, run_scan};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("scan-copy-leader-categories-{name}-{suffix}"))
    }

    fn write_executable(path: &PathBuf, contents: &str) {
        fs::write(path, contents).expect("script written");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("perms");
    }

    #[test]
    fn parse_args_accepts_category_scan_flags() {
        let options = parse_args(&[
            "--categories".into(),
            "SPORTS,CRYPTO".into(),
            "--limit".into(),
            "3".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
        ])
        .expect("parse");

        assert_eq!(options.categories, "SPORTS,CRYPTO");
        assert_eq!(options.limit, 3);
        assert_eq!(options.proxy.as_deref(), Some("http://127.0.0.1:7897"));
    }

    #[test]
    fn build_discover_args_forwards_category_and_proxy() {
        let options = parse_args(&[
            "--categories".into(),
            "SPORTS".into(),
            "--proxy".into(),
            "http://127.0.0.1:7897".into(),
        ])
        .expect("parse");
        let args = build_discover_args(&options, "SPORTS");

        assert!(args.contains(&"--category".to_string()));
        assert!(args.contains(&"SPORTS".to_string()));
        assert!(args.contains(&"--proxy".to_string()));
    }

    #[test]
    fn run_scan_summarizes_pass_and_reject_categories() {
        let root = unique_temp_dir("summary");
        let discovery_dir = root.join("discovery");
        fs::create_dir_all(&discovery_dir).expect("discovery dir created");
        let discover = root.join("discover_copy_leader");
        write_executable(
            &discover,
            concat!(
                "#!/usr/bin/env bash\n",
                "category=\"\"\n",
                "dir=\"\"\n",
                "while [[ $# -gt 0 ]]; do\n",
                "  case \"$1\" in\n",
                "    --category) category=\"$2\"; shift 2 ;;\n",
                "    --discovery-dir) dir=\"$2\"; shift 2 ;;\n",
                "    *) shift ;;\n",
                "  esac\n",
                "done\n",
                "mkdir -p \"$dir\"\n",
                "if [[ \"$category\" == \"SPORTS\" ]]; then\n",
                "  printf 'selected_wallet=0xgood\\nselected_score=88\\n'\n",
                "  printf 'wallet_filter_strategy=wallet_filter_v1\\nselected_wallet=0xgood\\n' > \"$dir/wallet-filter-v1-sports.txt\"\n",
                "else\n",
                "  printf 'wallet_filter_strategy=wallet_filter_v1\\nselected_wallet=none\\nrejection_reasons=maker_rebate_detected\\n' > \"$dir/wallet-filter-v1-crypto.txt\"\n",
                "  echo 'wallet_filter_v1 rejected every candidate' >&2\n",
                "  exit 1\n",
                "fi\n"
            ),
        );

        let options = parse_args(&[
            "--discovery-dir".into(),
            discovery_dir.display().to_string(),
            "--categories".into(),
            "SPORTS,CRYPTO".into(),
            "--discover-bin".into(),
            discover.display().to_string(),
        ])
        .expect("parse");

        let (summary_path, has_pass) = run_scan(&options).expect("scan should finish");
        assert!(has_pass);
        let summary = fs::read_to_string(summary_path).expect("summary exists");
        assert!(summary.contains("categories_scanned=2"));
        assert!(summary.contains("categories_passed=1"));
        assert!(summary.contains("categories_rejected=1"));
        assert!(summary.contains("best_pass_category=SPORTS"));
        assert!(summary.contains("best_rejected_category=CRYPTO"));
        assert!(summary.contains("best_rejected_wallet=none"));
        assert!(summary.contains("best_rejected_reason_count=0"));
        assert!(summary.contains("closest_rejected_categories=CRYPTO:0:-9223372036854775808"));
        assert!(summary.contains("== category SPORTS =="));
        assert!(summary.contains("status=passed"));
        assert!(summary.contains("== category CRYPTO =="));
        assert!(summary.contains("status=rejected"));

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
