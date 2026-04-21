use rust_copytrader::adapters::signing::AuthMaterial;
use rust_copytrader::config::{
    ActivityMode, CommandAdapterConfig, ExecutionAdapterConfig, LiveExecutionWiring, LiveModeGate,
    RootEnvLoadError, is_valid_evm_wallet,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let root = match parse_root_arg(&args) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    match render_live_bootstrap_report(&root) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to load rust-copytrader bootstrap from {}: {}",
                root.display(),
                format_root_env_error(&error)
            );
            ExitCode::from(1)
        }
    }
}

fn parse_root_arg(args: &[OsString]) -> Result<PathBuf, String> {
    let mut root = PathBuf::from(".");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--root" {
            let Some(value) = iter.next() else {
                return Err("usage: rust-copytrader [--root <path>]".to_string());
            };
            root = PathBuf::from(value);
            continue;
        }
        return Err("usage: rust-copytrader [--root <path>]".to_string());
    }
    Ok(root)
}

fn render_live_bootstrap_report(root: &Path) -> Result<String, RootEnvLoadError> {
    let execution_config = ExecutionAdapterConfig::from_root(root)?;
    let wiring = execution_config.live_execution_wiring();
    let (selected_leader_wallet, selected_leader_source) = selected_leader_context(root);
    let gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    let decision = format_live_bootstrap_decision(&gate);

    Ok([
        format!("root={}", root.display()),
        format!("selected_leader_wallet={selected_leader_wallet}"),
        format!("selected_leader_source={selected_leader_source}"),
        "requested_mode=live_listen".to_string(),
        format!("decision={decision}"),
        format!("live_mode_unlocked={}", gate.unlocked()),
        format!(
            "signing_command={}",
            wiring
                .as_ref()
                .map(|wiring| format_command(&wiring.signing))
                .unwrap_or_else(|| "disabled".to_string())
        ),
        format!(
            "submit_command={}",
            wiring
                .as_ref()
                .map(|wiring| format_command(&wiring.submit))
                .unwrap_or_else(|| "disabled".to_string())
        ),
        format!(
            "submit_base_url={}",
            optional_wiring_field(wiring.as_ref(), |wiring| &wiring.submit_base_url)
        ),
        format!(
            "submit_connect_timeout_ms={}",
            wiring
                .as_ref()
                .map(|wiring| wiring.submit_connect_timeout_ms.to_string())
                .unwrap_or_else(|| "disabled".to_string())
        ),
        format!(
            "submit_max_time_ms={}",
            wiring
                .as_ref()
                .map(|wiring| wiring.submit_max_time_ms.to_string())
                .unwrap_or_else(|| "disabled".to_string())
        ),
        "watch_copy_leader_activity_hint=cd rust-copytrader && cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --poll-count 1".to_string(),
        "run_copytrader_live_submit_gate_hint=cd rust-copytrader && cargo run --bin run_copytrader_live_submit_gate -- --root .. --max-total-exposure-usdc 100 --max-order-usdc 10 --account-snapshot runtime-verify-account/dashboard.json".to_string(),
        "run_copytrader_minmax_follow_hint=cd rust-copytrader && cargo run --bin run_copytrader_minmax_follow -- --root .. --user <wallet> --proxy http://127.0.0.1:7897".to_string(),
        "run_rust_minmax_follow_script_hint=bash scripts/run_rust_minmax_follow.sh --user <wallet>".to_string(),
        "run_rust_minmax_follow_live_script_hint=bash scripts/run_rust_minmax_follow_live.sh --user <wallet>".to_string(),
        "leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a".to_string(),
    ]
    .join("\n"))
}

fn selected_leader_context(root: &Path) -> (String, String) {
    env::var("COPYTRADER_DISCOVERY_WALLET")
        .ok()
        .and_then(|wallet| {
            is_valid_evm_wallet(&wallet)
                .then(|| (wallet, "env:COPYTRADER_DISCOVERY_WALLET".to_string()))
        })
        .or_else(|| {
            discovery_wallet_from_env_file(&root.join(".omx/discovery/selected-leader.env")).map(
                |wallet| {
                    (
                        wallet,
                        "file:.omx/discovery/selected-leader.env".to_string(),
                    )
                },
            )
        })
        .or_else(|| {
            AuthMaterial::from_root(root)
                .ok()
                .map(|material| (material.poly_address, "auth_material".to_string()))
        })
        .or_else(|| {
            env::var("POLY_ADDRESS")
                .ok()
                .map(|wallet| (wallet, "env:POLY_ADDRESS".to_string()))
        })
        .or_else(|| {
            env::var("SIGNER_ADDRESS")
                .ok()
                .map(|wallet| (wallet, "env:SIGNER_ADDRESS".to_string()))
        })
        .unwrap_or_else(|| {
            (
                "0x56687bf447db6ffa42ffe2204a05edaa20f55839".to_string(),
                "fallback".to_string(),
            )
        })
}

fn discovery_wallet_from_env_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(key, value)| match key.trim() {
            "COPYTRADER_DISCOVERY_WALLET" | "COPYTRADER_LEADER_WALLET" => {
                let value = value.trim();
                if value.is_empty() || !is_valid_evm_wallet(value) {
                    None
                } else {
                    Some(value.to_string())
                }
            }
            _ => None,
        })
}

fn format_live_bootstrap_decision(gate: &LiveModeGate) -> String {
    gate.blocked_reason()
        .map(|reason| format!("blocked:{reason}"))
        .unwrap_or_else(|| "live_listen".to_string())
}

fn optional_wiring_field(
    wiring: Option<&LiveExecutionWiring>,
    field: impl FnOnce(&LiveExecutionWiring) -> &str,
) -> String {
    wiring
        .map(field)
        .map(ToString::to_string)
        .unwrap_or_else(|| "disabled".to_string())
}

fn format_command(command: &CommandAdapterConfig) -> String {
    if command.args.is_empty() {
        return command.program.clone();
    }

    let mut parts = Vec::with_capacity(command.args.len() + 1);
    parts.push(command.program.clone());
    parts.extend(command.args.iter().cloned());
    parts.join(" ")
}

fn format_root_env_error(error: &RootEnvLoadError) -> String {
    match error {
        RootEnvLoadError::Io { path, error } => {
            format!("io error at {}: {error}", path.display())
        }
        RootEnvLoadError::MissingField(field) => format!("missing field {field}"),
        RootEnvLoadError::InvalidNumber { field, value } => {
            format!("invalid number for {field}: {value}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_root_arg, render_live_bootstrap_report, selected_leader_context};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rust-copytrader-main-{name}-{suffix}"))
    }

    #[test]
    fn parse_root_arg_accepts_root_flag() {
        let args = vec![
            std::ffi::OsString::from("--root"),
            std::ffi::OsString::from("/tmp/demo"),
        ];

        let root = parse_root_arg(&args).expect("root should parse");
        assert_eq!(root, PathBuf::from("/tmp/demo"));
    }

    #[test]
    fn bootstrap_report_defaults_to_blocked_without_helper_wiring() {
        let root = unique_temp_root("bootstrap-report-default");
        fs::create_dir_all(&root).expect("temp root created");

        let report = render_live_bootstrap_report(&root).expect("report should render");

        assert!(
            report.contains("selected_leader_wallet=0x56687bf447db6ffa42ffe2204a05edaa20f55839")
        );
        assert!(report.contains("selected_leader_source=fallback"));
        assert!(report.contains("requested_mode=live_listen"));
        assert!(report.contains("decision=blocked:activity_source_unverified"));
        assert!(report.contains("live_mode_unlocked=false"));
        assert!(report.contains("signing_command=disabled"));
        assert!(report.contains("submit_command=disabled"));
        assert!(report.contains("run_rust_minmax_follow_live_script_hint="));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn bootstrap_report_loads_repo_local_helper_wiring_without_unlocking_live_mode() {
        let root = unique_temp_root("bootstrap-report-wired");
        fs::create_dir_all(&root).expect("temp root created");
        fs::write(
            root.join(".env.local"),
            concat!(
                "RUST_COPYTRADER_SIGNING_PROGRAM=rust_sdk\n",
                "RUST_COPYTRADER_SUBMIT_PROGRAM=curl\n",
                "CLOB_HOST=https://clob.polymarket.com\n",
                "RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS=75\n",
                "RUST_COPYTRADER_SUBMIT_MAX_TIME_MS=150\n",
            ),
        )
        .expect(".env.local written");

        let report = render_live_bootstrap_report(&root).expect("report should render");

        assert!(report.contains("decision=blocked:activity_source_unverified"));
        assert!(report.contains("live_mode_unlocked=false"));
        assert!(report.contains("signing_command=rust_sdk"));
        assert!(report.contains("submit_command=curl"));
        assert!(report.contains("submit_base_url=https://clob.polymarket.com"));
        assert!(report.contains("submit_connect_timeout_ms=75"));
        assert!(report.contains("submit_max_time_ms=150"));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn selected_leader_prefers_selected_leader_env_file_when_present() {
        let root = unique_temp_root("selected-leader");
        fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir created");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0x11084005d88A0840b5F38F8731CCa9152BbD99F7\n",
        )
        .expect("selected leader env written");

        let (selected_wallet, selected_source) = selected_leader_context(&root);

        assert_eq!(
            selected_wallet,
            "0x11084005d88A0840b5F38F8731CCa9152BbD99F7"
        );
        assert_eq!(selected_source, "file:.omx/discovery/selected-leader.env");

        fs::remove_dir_all(root).expect("temp root removed");
    }
}
