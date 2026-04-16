use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::{Command, ExitCode, Output};

const BASE_URL: &str = "https://data-api.polymarket.com/v1/leaderboard";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    category: String,
    time_period: String,
    order_by: String,
    limit: usize,
    offset: usize,
    user: Option<String>,
    username: Option<String>,
    output: Option<String>,
    curl_bin: String,
    print_url: bool,
    print_curl: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            category: "OVERALL".to_string(),
            time_period: "DAY".to_string(),
            order_by: "PNL".to_string(),
            limit: 25,
            offset: 0,
            user: None,
            username: None,
            output: None,
            curl_bin: "curl".to_string(),
            print_url: false,
            print_curl: false,
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

    let url = build_url(&options);
    if options.print_url {
        println!("{url}");
        return ExitCode::SUCCESS;
    }
    if options.print_curl {
        println!(
            "{} {}",
            options.curl_bin,
            shell_join(&build_curl_args(&options))
        );
        return ExitCode::SUCCESS;
    }

    match run_request(&options) {
        Ok(output) => {
            if let Some(path) = &options.output {
                if let Err(error) = write_output_file(path, &output.stdout) {
                    eprintln!("failed to write {path}: {error}");
                    return ExitCode::from(1);
                }
                println!("saved_output={path}");
            } else if let Err(error) = io::stdout().write_all(&output.stdout) {
                eprintln!("failed to write response: {error}");
                return ExitCode::from(1);
            }
            if let Err(error) = io::stderr().write_all(&output.stderr) {
                eprintln!("failed to write stderr: {error}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(RequestError::Command(error)) => {
            eprintln!("failed to execute {}: {error}", options.curl_bin);
            ExitCode::from(1)
        }
        Err(RequestError::Curl(output)) => {
            let _ = io::stderr().write_all(&output.stderr);
            let _ = io::stderr().write_all(&output.stdout);
            ExitCode::from(output.status.code().unwrap_or(1) as u8)
        }
    }
}

fn print_usage() {
    println!(
        "usage: fetch_trader_leaderboard [--category <value>] [--time-period <value>] [--order-by <value>] [--limit <n>] [--offset <n>] [--user <wallet>] [--username <name>] [--output <path>] [--curl-bin <path>] [--print-url] [--print-curl]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--category" => options.category = next_value(&mut iter, arg)?,
            "--time-period" => options.time_period = next_value(&mut iter, arg)?,
            "--order-by" => options.order_by = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--offset" => options.offset = parse_usize(&next_value(&mut iter, arg)?, "offset")?,
            "--user" => options.user = Some(next_value(&mut iter, arg)?),
            "--username" => options.username = Some(next_value(&mut iter, arg)?),
            "--output" => options.output = Some(next_value(&mut iter, arg)?),
            "--curl-bin" => options.curl_bin = next_value(&mut iter, arg)?,
            "--print-url" => options.print_url = true,
            "--print-curl" => options.print_curl = true,
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

fn build_url(options: &Options) -> String {
    let mut params = vec![
        format!("category={}", encode_component(&options.category)),
        format!("timePeriod={}", encode_component(&options.time_period)),
        format!("orderBy={}", encode_component(&options.order_by)),
        format!("limit={}", options.limit),
        format!("offset={}", options.offset),
    ];
    if let Some(user) = &options.user {
        params.push(format!("user={}", encode_component(user)));
    }
    if let Some(username) = &options.username {
        params.push(format!("userName={}", encode_component(username)));
    }
    format!("{BASE_URL}?{}", params.join("&"))
}

fn build_curl_args(options: &Options) -> Vec<String> {
    vec![
        "--silent".to_string(),
        "--show-error".to_string(),
        "--fail-with-body".to_string(),
        "-A".to_string(),
        "Mozilla/5.0".to_string(),
        "-H".to_string(),
        "Accept: application/json".to_string(),
        build_url(options),
    ]
}

#[derive(Debug)]
enum RequestError {
    Command(io::Error),
    Curl(Output),
}

fn write_output_file(path: &str, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

fn run_request(options: &Options) -> Result<Output, RequestError> {
    let output = Command::new(&options.curl_bin)
        .args(build_curl_args(options))
        .output()
        .map_err(RequestError::Command)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(RequestError::Curl(output))
    }
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=?&%".contains(ch))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\"'\"'"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{Options, build_curl_args, build_url, parse_args, run_request, write_output_file};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("fetch-trader-leaderboard-{name}-{suffix}"))
    }

    #[test]
    fn parses_and_builds_leaderboard_url() {
        let options = parse_args(&[
            "--category".into(),
            "OVERALL".into(),
            "--time-period".into(),
            "WEEK".into(),
            "--order-by".into(),
            "VOL".into(),
            "--limit".into(),
            "10".into(),
            "--offset".into(),
            "20".into(),
            "--username".into(),
            "alice".into(),
        ])
        .expect("args parse");

        let url = build_url(&options);

        assert!(url.contains("category=OVERALL"));
        assert!(url.contains("timePeriod=WEEK"));
        assert!(url.contains("orderBy=VOL"));
        assert!(url.contains("limit=10"));
        assert!(url.contains("offset=20"));
        assert!(url.contains("userName=alice"));
    }

    #[test]
    fn curl_args_use_json_accept_and_user_agent() {
        let args = build_curl_args(&Options::default());

        assert!(args.contains(&"--fail-with-body".to_string()));
        assert!(args.contains(&"Accept: application/json".to_string()));
        assert!(args.contains(&"Mozilla/5.0".to_string()));
    }

    #[test]
    fn parse_args_supports_print_flags() {
        let options = parse_args(&["--print-url".into(), "--print-curl".into()]).expect("parse");
        assert!(options.print_url);
        assert!(options.print_curl);
    }

    #[test]
    fn parse_args_supports_output_path() {
        let options =
            parse_args(&["--output".into(), "/tmp/leaderboard.json".into()]).expect("parse");
        assert_eq!(options.output.as_deref(), Some("/tmp/leaderboard.json"));
    }

    #[test]
    fn run_request_captures_successful_stdout_for_output_files() {
        let root = unique_temp_dir("output");
        fs::create_dir_all(&root).expect("temp dir created");
        let curl_stub = root.join("curl-stub.sh");
        fs::write(&curl_stub, "#!/usr/bin/env bash\nprintf '{\"rows\":[1]}'\n")
            .expect("stub written");
        let mut perms = fs::metadata(&curl_stub).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&curl_stub, perms).expect("perms");

        let options = parse_args(&[
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--output".into(),
            root.join("leaderboard.json").display().to_string(),
        ])
        .expect("parse");

        let output = run_request(&options).expect("request should succeed");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{\"rows\":[1]}");

        fs::remove_dir_all(root).expect("temp dir removed");
    }

    #[test]
    fn write_output_file_creates_parent_directories() {
        let root = unique_temp_dir("nested-output");
        let output_path = root.join("nested").join("leaderboard.json");

        write_output_file(output_path.to_str().expect("utf8 path"), br#"{"rows":[1]}"#)
            .expect("write should succeed");

        assert_eq!(
            fs::read_to_string(&output_path).expect("file exists"),
            "{\"rows\":[1]}"
        );

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
