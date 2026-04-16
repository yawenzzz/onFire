use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::{Command, ExitCode, Output};

const BASE_URL: &str = "https://data-api.polymarket.com/activity";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    user: String,
    limit: usize,
    offset: usize,
    types: Vec<String>,
    market: Vec<String>,
    event_id: Vec<String>,
    start: Option<i64>,
    end: Option<i64>,
    sort_by: String,
    sort_direction: String,
    side: Option<String>,
    output: Option<String>,
    curl_bin: String,
    print_url: bool,
    print_curl: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            user: String::new(),
            limit: 100,
            offset: 0,
            types: Vec::new(),
            market: Vec::new(),
            event_id: Vec::new(),
            start: None,
            end: None,
            sort_by: "TIMESTAMP".to_string(),
            sort_direction: "DESC".to_string(),
            side: None,
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
                if let Err(error) = fs::write(path, &output.stdout) {
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
        "usage: fetch_user_activity --user <wallet> [--type <value>] [--market <slug>] [--event-id <value>] [--start <ts>] [--end <ts>] [--side <BUY|SELL>] [--limit <n>] [--offset <n>] [--sort-by <field>] [--sort-direction <ASC|DESC>] [--output <path>] [--curl-bin <path>] [--print-url] [--print-curl]"
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--user" => options.user = next_value(&mut iter, arg)?,
            "--limit" => options.limit = parse_usize(&next_value(&mut iter, arg)?, "limit")?,
            "--offset" => options.offset = parse_usize(&next_value(&mut iter, arg)?, "offset")?,
            "--type" => options.types.push(next_value(&mut iter, arg)?),
            "--market" => options.market.push(next_value(&mut iter, arg)?),
            "--event-id" => options.event_id.push(next_value(&mut iter, arg)?),
            "--start" => options.start = Some(parse_i64(&next_value(&mut iter, arg)?, "start")?),
            "--end" => options.end = Some(parse_i64(&next_value(&mut iter, arg)?, "end")?),
            "--sort-by" => options.sort_by = next_value(&mut iter, arg)?,
            "--sort-direction" => options.sort_direction = next_value(&mut iter, arg)?,
            "--side" => options.side = Some(next_value(&mut iter, arg)?),
            "--output" => options.output = Some(next_value(&mut iter, arg)?),
            "--curl-bin" => options.curl_bin = next_value(&mut iter, arg)?,
            "--print-url" => options.print_url = true,
            "--print-curl" => options.print_curl = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if options.user.trim().is_empty() {
        return Err("missing required --user".to_string());
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

fn parse_i64(value: &str, field: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
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
        format!("user={}", encode_component(&options.user)),
        format!("limit={}", options.limit),
        format!("offset={}", options.offset),
        format!("sortBy={}", encode_component(&options.sort_by)),
        format!(
            "sortDirection={}",
            encode_component(&options.sort_direction)
        ),
    ];
    if !options.types.is_empty() {
        params.push(format!(
            "type={}",
            encode_component(&options.types.join(","))
        ));
    }
    if !options.market.is_empty() {
        params.push(format!(
            "market={}",
            encode_component(&options.market.join(","))
        ));
    }
    if !options.event_id.is_empty() {
        params.push(format!(
            "eventId={}",
            encode_component(&options.event_id.join(","))
        ));
    }
    if let Some(start) = options.start {
        params.push(format!("start={start}"));
    }
    if let Some(end) = options.end {
        params.push(format!("end={end}"));
    }
    if let Some(side) = &options.side {
        params.push(format!("side={}", encode_component(side)));
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
    use super::{build_curl_args, build_url, parse_args, run_request};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("fetch-user-activity-{name}-{suffix}"))
    }

    #[test]
    fn user_activity_requires_user_and_keeps_filters() {
        let options = parse_args(&[
            "--user".into(),
            "0xabc".into(),
            "--type".into(),
            "TRADE".into(),
            "--side".into(),
            "BUY".into(),
            "--limit".into(),
            "10".into(),
        ])
        .expect("args parse");

        let url = build_url(&options);

        assert!(url.contains("user=0xabc"));
        assert!(url.contains("type=TRADE"));
        assert!(url.contains("side=BUY"));
        assert!(url.contains("limit=10"));
    }

    #[test]
    fn curl_args_include_user_agent_and_json_accept() {
        let options = parse_args(&["--user".into(), "0xabc".into()]).expect("args parse");
        let args = build_curl_args(&options);

        assert!(args.contains(&"--fail-with-body".to_string()));
        assert!(args.contains(&"Accept: application/json".to_string()));
        assert!(args.contains(&"Mozilla/5.0".to_string()));
    }

    #[test]
    fn parse_args_supports_print_flags() {
        let options =
            parse_args(&["--user".into(), "0xabc".into(), "--print-url".into()]).expect("parse");
        assert!(options.print_url);
    }

    #[test]
    fn parse_args_supports_output_path() {
        let options = parse_args(&[
            "--user".into(),
            "0xabc".into(),
            "--output".into(),
            "/tmp/activity.json".into(),
        ])
        .expect("parse");
        assert_eq!(options.output.as_deref(), Some("/tmp/activity.json"));
    }

    #[test]
    fn run_request_captures_successful_stdout_for_output_files() {
        let root = unique_temp_dir("output");
        fs::create_dir_all(&root).expect("temp dir created");
        let curl_stub = root.join("curl-stub.sh");
        fs::write(
            &curl_stub,
            "#!/usr/bin/env bash\nprintf '{\"fills\":[1]}'\n",
        )
        .expect("stub written");
        let mut perms = fs::metadata(&curl_stub).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&curl_stub, perms).expect("perms");

        let options = parse_args(&[
            "--user".into(),
            "0xabc".into(),
            "--curl-bin".into(),
            curl_stub.display().to_string(),
            "--output".into(),
            root.join("activity.json").display().to_string(),
        ])
        .expect("parse");

        let output = run_request(&options).expect("request should succeed");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{\"fills\":[1]}");

        fs::remove_dir_all(root).expect("temp dir removed");
    }
}
