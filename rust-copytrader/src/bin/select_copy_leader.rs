use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Options {
    wallet: Option<String>,
    leaderboard: Option<String>,
    activity: Option<String>,
    index: usize,
    output: Option<String>,
    print_wallet: bool,
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

    let wallet = match resolve_wallet(&options) {
        Ok(wallet) => wallet,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    if options.print_wallet {
        println!("{wallet}");
        return ExitCode::SUCCESS;
    }

    let output = render_env_output(&wallet, &options);
    match &options.output {
        Some(path) => match write_output_file(path, output.as_bytes()) {
            Ok(()) => {
                println!("saved_output={path}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to write {path}: {error}");
                ExitCode::from(1)
            }
        },
        None => {
            print!("{output}");
            ExitCode::SUCCESS
        }
    }
}

fn print_usage() {
    println!(
        "usage: select_copy_leader [--wallet <wallet> | --leaderboard <path> | --activity <path>] [--index <n>] [--output <path>] [--print-wallet]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--wallet" => options.wallet = Some(next_value(&mut iter, arg)?),
            "--leaderboard" => options.leaderboard = Some(next_value(&mut iter, arg)?),
            "--activity" => options.activity = Some(next_value(&mut iter, arg)?),
            "--index" => options.index = parse_usize(&next_value(&mut iter, arg)?, "index")?,
            "--output" => options.output = Some(next_value(&mut iter, arg)?),
            "--print-wallet" => options.print_wallet = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let source_count = usize::from(options.wallet.is_some())
        + usize::from(options.leaderboard.is_some())
        + usize::from(options.activity.is_some());
    if source_count == 0 {
        return Err("missing required --wallet, --leaderboard, or --activity".to_string());
    }
    if source_count > 1 {
        return Err("use exactly one of --wallet, --leaderboard, or --activity".to_string());
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

fn resolve_wallet(options: &Options) -> Result<String, String> {
    if let Some(wallet) = &options.wallet {
        return Ok(wallet.trim().to_string());
    }

    if let Some(leaderboard_path) = &options.leaderboard {
        let content = fs::read_to_string(leaderboard_path)
            .map_err(|error| format!("failed to read {leaderboard_path}: {error}"))?;
        return extract_wallet_from_json(&content, options.index).ok_or_else(|| {
            format!(
                "failed to extract wallet at index {} from {}",
                options.index, leaderboard_path
            )
        });
    }

    let activity_path = options
        .activity
        .as_ref()
        .ok_or_else(|| "missing activity path".to_string())?;
    let content = fs::read_to_string(activity_path)
        .map_err(|error| format!("failed to read {activity_path}: {error}"))?;
    extract_wallet_from_json(&content, options.index).ok_or_else(|| {
        format!(
            "failed to extract wallet at index {} from {}",
            options.index, activity_path
        )
    })
}

fn extract_wallet_from_json(content: &str, index: usize) -> Option<String> {
    let fields = ["proxyWallet", "wallet", "address", "user"];
    let mut wallets = Vec::new();

    for field in fields {
        let needle = format!("\"{field}\"");
        let mut remaining = content;
        while let Some(start) = remaining.find(&needle) {
            remaining = &remaining[start + needle.len()..];
            let Some(colon) = remaining.find(':') else {
                break;
            };
            remaining = &remaining[colon + 1..];
            let trimmed = remaining.trim_start();
            if !trimmed.starts_with('"') {
                remaining = trimmed;
                continue;
            }
            let trimmed = &trimmed[1..];
            let Some(end) = trimmed.find('"') else {
                break;
            };
            let candidate = &trimmed[..end];
            if looks_like_wallet(candidate) && !wallets.iter().any(|seen| seen == candidate) {
                wallets.push(candidate.to_string());
            }
            remaining = &trimmed[end + 1..];
        }
    }

    wallets.get(index).cloned()
}

fn looks_like_wallet(value: &str) -> bool {
    value.starts_with("0x") && value.len() >= 6
}

fn render_env_output(wallet: &str, options: &Options) -> String {
    let source = if let Some(path) = &options.leaderboard {
        format!("leaderboard:{}#{}", path, options.index)
    } else if let Some(path) = &options.activity {
        format!("activity:{}#{}", path, options.index)
    } else {
        "wallet".to_string()
    };

    [
        format!("COPYTRADER_DISCOVERY_WALLET={wallet}"),
        format!("COPYTRADER_LEADER_WALLET={wallet}"),
        format!("COPYTRADER_SELECTED_FROM={source}"),
    ]
    .join("\n")
        + "\n"
}

fn write_output_file(path: &str, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        extract_wallet_from_json, parse_args, render_env_output, resolve_wallet, write_output_file,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("select-copy-leader-{name}-{suffix}"))
    }

    #[test]
    fn parse_args_accepts_wallet_and_output() {
        let options = parse_args(&[
            "--wallet".into(),
            "0xabc".into(),
            "--output".into(),
            "/tmp/selected.env".into(),
            "--print-wallet".into(),
        ])
        .expect("parse");

        assert_eq!(options.wallet.as_deref(), Some("0xabc"));
        assert_eq!(options.output.as_deref(), Some("/tmp/selected.env"));
        assert!(options.print_wallet);
    }

    #[test]
    fn extract_wallet_from_leaderboard_prefers_wallet_like_fields() {
        let json = r#"[
          {"rank":1,"proxyWallet":"0xleader1","volume":1},
          {"rank":2,"wallet":"0xleader2","volume":2}
        ]"#;

        assert_eq!(
            extract_wallet_from_json(json, 0).as_deref(),
            Some("0xleader1")
        );
        assert_eq!(
            extract_wallet_from_json(json, 1).as_deref(),
            Some("0xleader2")
        );
    }

    #[test]
    fn extract_wallet_from_activity_prefers_proxy_wallet_fields() {
        let json = r#"[
          {"proxyWallet":"0xleader3","side":"BUY"},
          {"user":"0xleader4","side":"SELL"}
        ]"#;

        assert_eq!(
            extract_wallet_from_json(json, 0).as_deref(),
            Some("0xleader3")
        );
        assert_eq!(
            extract_wallet_from_json(json, 1).as_deref(),
            Some("0xleader4")
        );
    }

    #[test]
    fn resolve_wallet_reads_leaderboard_file() {
        let root = unique_temp_dir("leaderboard");
        fs::create_dir_all(&root).expect("temp dir created");
        let leaderboard = root.join("leaderboard.json");
        fs::write(
            &leaderboard,
            r#"[{"proxyWallet":"0xleader1"},{"proxyWallet":"0xleader2"}]"#,
        )
        .expect("leaderboard written");

        let options = parse_args(&[
            "--leaderboard".into(),
            leaderboard.display().to_string(),
            "--index".into(),
            "1".into(),
        ])
        .expect("parse");

        assert_eq!(resolve_wallet(&options).expect("wallet"), "0xleader2");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn resolve_wallet_reads_activity_file() {
        let root = unique_temp_dir("activity");
        fs::create_dir_all(&root).expect("temp dir created");
        let activity = root.join("activity.json");
        fs::write(
            &activity,
            r#"[{"proxyWallet":"0xleader3"},{"proxyWallet":"0xleader4"}]"#,
        )
        .expect("activity written");

        let options = parse_args(&[
            "--activity".into(),
            activity.display().to_string(),
            "--index".into(),
            "0".into(),
        ])
        .expect("parse");

        assert_eq!(resolve_wallet(&options).expect("wallet"), "0xleader3");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn render_env_output_contains_selected_wallet_and_source() {
        let options = parse_args(&["--wallet".into(), "0xleader1".into()]).expect("parse");
        let rendered = render_env_output("0xleader1", &options);

        assert!(rendered.contains("COPYTRADER_DISCOVERY_WALLET=0xleader1"));
        assert!(rendered.contains("COPYTRADER_LEADER_WALLET=0xleader1"));
        assert!(rendered.contains("COPYTRADER_SELECTED_FROM=wallet"));
    }

    #[test]
    fn render_env_output_uses_activity_source_when_selected_from_activity() {
        let options = parse_args(&[
            "--activity".into(),
            "/tmp/activity.json".into(),
            "--index".into(),
            "1".into(),
        ])
        .expect("parse");
        let rendered = render_env_output("0xleader9", &options);

        assert!(rendered.contains("COPYTRADER_SELECTED_FROM=activity:/tmp/activity.json#1"));
    }

    #[test]
    fn write_output_file_creates_parent_directories() {
        let root = unique_temp_dir("output");
        let output = root.join("nested").join("selected.env");

        write_output_file(
            output.to_str().expect("utf8 path"),
            b"COPYTRADER_DISCOVERY_WALLET=0xleader1\n",
        )
        .expect("write should succeed");

        assert_eq!(
            fs::read_to_string(&output).expect("env file exists"),
            "COPYTRADER_DISCOVERY_WALLET=0xleader1\n"
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
