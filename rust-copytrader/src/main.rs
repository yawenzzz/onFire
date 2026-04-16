use rust_copytrader::adapters::auth::AuthRuntimeState;
use rust_copytrader::adapters::http_submit::{HttpSubmitter, OrderBatchRequest, OrderType};
use rust_copytrader::adapters::signing::{
    AuthMaterial, CommandL2HeaderSigner, CommandOrderSigner, OrderSigner, StdSigningCommandRunner,
    UnsignedOrderPayload, prepare_l2_auth_headers, prepare_signed_order,
};
use rust_copytrader::app::{
    BootstrapDecision, RuntimeBootstrap, RuntimeSession, RuntimeSessionRecorder, SessionOutcome,
};
use rust_copytrader::config::{
    ActivityMode, CommandAdapterConfig, LiveExecutionWiring, LiveModeGate, RootEnvLoadError,
};
use rust_copytrader::domain::budget::LatencyBudget;
use rust_copytrader::replay::fixture::ReplayFixture;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let root = match parse_root_arg(&args) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    let smoke_mode = args.iter().any(|arg| arg == "--smoke-helper");
    let runtime_smoke_mode = args.iter().any(|arg| arg == "--smoke-runtime");
    let operator_demo_mode = args.iter().any(|arg| arg == "--operator-demo");

    match if operator_demo_mode {
        render_operator_demo_report(&root)
    } else if runtime_smoke_mode {
        render_runtime_smoke_report(&root)
    } else if smoke_mode {
        render_live_helper_smoke_report(&root)
    } else {
        render_live_bootstrap_report(&root)
    } {
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
        if arg == "--smoke-helper" || arg == "--smoke-runtime" || arg == "--operator-demo" {
            continue;
        }
        if arg == "--root" {
            let Some(value) = iter.next() else {
                return Err(
                    "usage: rust-copytrader [--smoke-helper|--smoke-runtime|--operator-demo] [--root <path>]"
                        .to_string(),
                );
            };
            root = PathBuf::from(value);
            continue;
        }
        return Err(
            "usage: rust-copytrader [--smoke-helper|--smoke-runtime|--operator-demo] [--root <path>]".to_string(),
        );
    }
    Ok(root)
}

fn render_live_bootstrap_report(root: &Path) -> Result<String, RootEnvLoadError> {
    let bootstrap = RuntimeBootstrap::from_root(
        ActivityMode::LiveListen,
        LiveModeGate::for_mode(ActivityMode::LiveListen),
        root,
    )?;
    let decision = bootstrap.decide();
    let wiring = bootstrap.live_execution_wiring();
    let l2_helper = bootstrap.live_l2_header_helper();
    let (selected_leader_wallet, selected_leader_source) = selected_leader_context(root);

    Ok([
        format!("root={}", root.display()),
        format!("selected_leader_wallet={selected_leader_wallet}"),
        format!("selected_leader_source={selected_leader_source}"),
        "requested_mode=live_listen".to_string(),
        format!("decision={}", format_bootstrap_decision(&decision)),
        format!(
            "live_mode_unlocked={}",
            matches!(decision, BootstrapDecision::LiveListen)
        ),
        format!(
            "signing_command={}",
            wiring
                .as_ref()
                .map(|wiring| format_command(&wiring.signing))
                .unwrap_or_else(|| "disabled".to_string())
        ),
        format!(
            "l2_header_helper={}",
            l2_helper
                .as_ref()
                .map(format_command)
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
    ]
    .join("\n"))
}

fn render_live_helper_smoke_report(root: &Path) -> Result<String, RootEnvLoadError> {
    let bootstrap = RuntimeBootstrap::from_root(
        ActivityMode::LiveListen,
        LiveModeGate::for_mode(ActivityMode::LiveListen),
        root,
    )?;
    let mut lines = vec![
        "mode=helper-smoke".to_string(),
        format!("root={}", root.display()),
        render_live_bootstrap_report(root)?,
    ];

    let Some(wiring) = bootstrap.live_execution_wiring() else {
        lines.push("helper_smoke=disabled".to_string());
        return Ok(lines.join("\n"));
    };
    let Some(l2_helper) = bootstrap.live_l2_header_helper() else {
        lines.push("helper_smoke=l2_helper_disabled".to_string());
        return Ok(lines.join("\n"));
    };

    let material = AuthMaterial::from_root(root)?;
    let signing_command = rebase_repo_local_command(&wiring.signing);
    let l2_helper_command = rebase_repo_local_command(&l2_helper);
    let mut order_signer = CommandOrderSigner::new(
        signing_command.program.clone(),
        signing_command.args.clone(),
        StdSigningCommandRunner,
    );
    let mut l2_signer = CommandL2HeaderSigner::new(
        l2_helper_command.program.clone(),
        l2_helper_command.args.clone(),
        StdSigningCommandRunner,
    );

    let unsigned = sample_unsigned_order();
    let signed = order_signer
        .sign_order(&unsigned, &material)
        .map_err(|error| RootEnvLoadError::Io {
            path: root.to_path_buf(),
            error: format!("order signing helper failed: {error:?}"),
        })?;
    let l2_headers = prepare_l2_auth_headers(&material, sample_l2_payload(), &mut l2_signer)
        .map_err(|error| RootEnvLoadError::Io {
            path: root.to_path_buf(),
            error: format!("l2 helper failed: {error:?}"),
        })?;
    let batch = OrderBatchRequest::single(
        prepare_signed_order(
            &material,
            unsigned,
            "owner-uuid",
            OrderType::Gtc,
            false,
            &mut order_signer,
        )
        .map_err(|error| RootEnvLoadError::Io {
            path: root.to_path_buf(),
            error: format!("signed order envelope failed: {error:?}"),
        })?,
    );
    let auth = AuthRuntimeState::new(
        true,
        true,
        true,
        material.signature_type,
        material.funder.is_some(),
    );
    let submit_preview = HttpSubmitter::from_live_execution_wiring(&wiring)
        .map_err(|error| RootEnvLoadError::Io {
            path: root.to_path_buf(),
            error: format!("submitter build failed: {error:?}"),
        })?
        .preview_command(&auth, &l2_headers, &batch)
        .map_err(|error| RootEnvLoadError::Io {
            path: root.to_path_buf(),
            error: format!("submit preview failed: {error:?}"),
        })?;

    lines.push("helper_smoke=ok".to_string());
    lines.push(format!("order_signature={}", signed.signature));
    lines.push(format!("order_salt={}", signed.salt));
    lines.push(format!("l2_signature={}", l2_headers.poly_signature));
    lines.push(format!("l2_timestamp={}", l2_headers.poly_timestamp));
    lines.push(format!("submit_preview_program={}", submit_preview.program));
    lines.push(format!(
        "submit_preview_args={}",
        submit_preview.args.join(" ")
    ));
    Ok(lines.join("\n"))
}

fn render_runtime_smoke_report(root: &Path) -> Result<String, RootEnvLoadError> {
    let mut lines = vec!["mode=runtime-smoke".to_string()];
    lines.extend(
        render_live_helper_smoke_report(root)?
            .lines()
            .map(ToString::to_string),
    );

    let fixture = ReplayFixture::success_buy_follow();
    let submit_budget = LatencyBudget::new(200);
    let mut session = RuntimeSession::from_root(
        ActivityMode::Replay,
        LiveModeGate::for_mode(ActivityMode::Replay),
        root,
    )?;
    let outcome = session.process_replay(&fixture);
    let smoke_root = root.join(".omx").join("runtime-smoke");
    let session_id = format!(
        "helper-smoke-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    );
    let mut recorder = RuntimeSessionRecorder::new(&smoke_root, &session_id, 3, 32);
    let artifacts = recorder
        .persist(&session)
        .map_err(|error| RootEnvLoadError::Io {
            path: smoke_root,
            error: error.to_string(),
        })?;

    lines.push(format!(
        "session_outcome={}",
        format_session_outcome(&outcome)
    ));
    lines.push(format!(
        "replay_submit_elapsed_ms={}",
        fixture.submit_elapsed_ms()
    ));
    lines.push(format!(
        "replay_verified_elapsed_ms={}",
        fixture
            .verification
            .observed_at_ms()
            .saturating_sub(fixture.activity.observed_at_ms)
    ));
    lines.push(format!(
        "submit_hard_budget_ms={}",
        submit_budget.hard_limit_ms()
    ));
    lines.push(format!(
        "submit_budget_headroom_ms={}",
        submit_budget
            .remaining_ms(fixture.submit_elapsed_ms())
            .unwrap_or(0)
    ));
    if let Some(snapshot) = session.snapshot() {
        lines.push(format!("runtime_mode={}", snapshot.runtime.mode));
        lines.push(format!(
            "last_submit_status={}",
            snapshot.runtime.last_submit_status
        ));
        lines.push(format!(
            "last_total_elapsed_ms={}",
            snapshot.runtime.last_total_elapsed_ms
        ));
    }
    lines.push(format!("session_id={session_id}"));
    lines.push(format!(
        "latest_snapshot_path={}",
        artifacts.latest_snapshot_path.display()
    ));
    lines.push(format!("report_path={}", artifacts.report_path.display()));
    lines.push(format!("summary_path={}", artifacts.summary_path.display()));
    Ok(lines.join("\n"))
}

fn render_operator_demo_report(root: &Path) -> Result<String, RootEnvLoadError> {
    let mut lines = vec!["mode=operator-demo".to_string()];
    lines.extend(
        render_runtime_smoke_report(root)?
            .lines()
            .map(ToString::to_string),
    );
    let (discovery_wallet, discovery_source) = selected_leader_context(root);
    lines.push(format!("selected_leader_wallet={discovery_wallet}"));
    lines.push(format!("selected_leader_source={discovery_source}"));
    lines.push(
        "leaderboard_hint=cd rust-copytrader && cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20".to_string(),
    );
    lines.push(format!(
        "activity_hint=cd rust-copytrader && cargo run --bin fetch_user_activity -- --user {} --type TRADE --limit 20",
        discovery_wallet
    ));
    lines.push(format!(
        "leaderboard_preview_url={}",
        default_leaderboard_preview_url()
    ));
    lines.push(format!(
        "leaderboard_preview_curl={}",
        default_leaderboard_preview_curl()
    ));
    lines.push(format!(
        "activity_preview_url={}",
        default_activity_preview_url(&discovery_wallet)
    ));
    lines.push(format!(
        "activity_preview_curl={}",
        default_activity_preview_curl(&discovery_wallet)
    ));
    lines.push(format!(
        "leaderboard_capture_hint=cd rust-copytrader && cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --output {}",
        shell_path_for_report("../.omx/discovery/leaderboard-overall-day-pnl.json")
    ));
    lines.push(format!(
        "activity_capture_hint=cd rust-copytrader && cargo run --bin fetch_user_activity -- --user {} --type TRADE --limit 20 --output {}",
        discovery_wallet,
        shell_path_for_report(format!(
            "../.omx/discovery/activity-{}-trade.json",
            sanitize_for_filename(&discovery_wallet)
        ))
    ));
    lines.push(format!(
        "leader_selection_hint=cd rust-copytrader && cargo run --bin select_copy_leader -- --leaderboard {} --output {}",
        shell_path_for_report("../.omx/discovery/leaderboard-overall-day-pnl.json"),
        shell_path_for_report("../.omx/discovery/selected-leader.env")
    ));
    lines.push(
        "leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a"
            .to_string(),
    );
    lines.push(
        "note=public discovery commands are read-only and may still fail due to remote access controls"
            .to_string(),
    );

    let operator_root = root.join(".omx").join("operator-demo");
    fs::create_dir_all(&operator_root).map_err(|error| RootEnvLoadError::Io {
        path: operator_root.clone(),
        error: error.to_string(),
    })?;
    let run_id = format!(
        "operator-demo-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    );
    let report_path = operator_root.join(format!("{run_id}.txt"));
    let latest_path = operator_root.join("latest.txt");
    lines.push(format!(
        "operator_demo_report_path={}",
        report_path.display()
    ));
    lines.push(format!(
        "operator_demo_latest_path={}",
        latest_path.display()
    ));

    let report = lines.join("\n");
    fs::write(&report_path, &report).map_err(|error| RootEnvLoadError::Io {
        path: report_path.clone(),
        error: error.to_string(),
    })?;
    fs::write(&latest_path, &report).map_err(|error| RootEnvLoadError::Io {
        path: latest_path.clone(),
        error: error.to_string(),
    })?;

    Ok(report)
}

fn sample_unsigned_order() -> UnsignedOrderPayload {
    UnsignedOrderPayload {
        taker: "0x0000000000000000000000000000000000000000".into(),
        token_id: "12345".into(),
        maker_amount: "1000000".into(),
        taker_amount: "2000000".into(),
        side: "BUY".into(),
        expiration: "1735689600".into(),
        nonce: "7".into(),
        fee_rate_bps: "30".into(),
    }
}

fn sample_l2_payload() -> rust_copytrader::adapters::signing::L2HeaderSigningPayload {
    rust_copytrader::adapters::signing::L2HeaderSigningPayload {
        method: "POST".into(),
        request_path: "/orders".into(),
        body: "{\"owner\":\"owner-uuid\"}".into(),
    }
}

fn format_session_outcome(outcome: &SessionOutcome) -> String {
    match outcome {
        SessionOutcome::Blocked(reason) => format!("blocked:{reason}"),
        SessionOutcome::Processed => "processed".to_string(),
    }
}

fn selected_leader_context(root: &Path) -> (String, String) {
    env::var("COPYTRADER_DISCOVERY_WALLET")
        .ok()
        .map(|wallet| (wallet, "env:COPYTRADER_DISCOVERY_WALLET".to_string()))
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
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            }
            _ => None,
        })
}

fn default_leaderboard_preview_url() -> String {
    "https://data-api.polymarket.com/v1/leaderboard?category=OVERALL&timePeriod=DAY&orderBy=PNL&limit=20&offset=0".to_string()
}

fn default_leaderboard_preview_curl() -> String {
    format!(
        "curl --silent --show-error --fail-with-body -A Mozilla/5.0 -H 'Accept: application/json' {}",
        default_leaderboard_preview_url()
    )
}

fn default_activity_preview_url(user: &str) -> String {
    format!(
        "https://data-api.polymarket.com/activity?user={}&limit=20&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type=TRADE",
        encode_url_component(user)
    )
}

fn default_activity_preview_curl(user: &str) -> String {
    format!(
        "curl --silent --show-error --fail-with-body -A Mozilla/5.0 -H 'Accept: application/json' {}",
        default_activity_preview_url(user)
    )
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

fn shell_path_for_report(path: impl AsRef<str>) -> String {
    path.as_ref().to_string()
}

fn encode_url_component(value: &str) -> String {
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

fn format_bootstrap_decision(decision: &BootstrapDecision) -> String {
    match decision {
        BootstrapDecision::LiveListen => "live_listen".to_string(),
        BootstrapDecision::ShadowPoll => "shadow_poll".to_string(),
        BootstrapDecision::Replay => "replay".to_string(),
        BootstrapDecision::Blocked(reason) => format!("blocked:{reason}"),
    }
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

fn rebase_repo_local_command(command: &CommandAdapterConfig) -> CommandAdapterConfig {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo manifest dir should have repo root parent");
    let mut rebased = command.clone();
    rebased.args = command
        .args
        .iter()
        .map(|arg| {
            if arg.starts_with("scripts/") {
                repo_root.join(arg).display().to_string()
            } else {
                arg.clone()
            }
        })
        .collect();
    rebased
}

#[cfg(test)]
mod tests {
    use super::{
        render_live_bootstrap_report, render_live_helper_smoke_report, render_operator_demo_report,
        render_runtime_smoke_report,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rust-copytrader-main-{name}-{suffix}"))
    }

    fn write_stub_sdk(root: &std::path::Path) -> PathBuf {
        let stub_root = root.join("stubs");
        let files = [
            ("py_clob_client/__init__.py", ""),
            (
                "py_clob_client/config.py",
                concat!(
                    "from types import SimpleNamespace\n",
                    "def get_contract_config(chain_id, neg_risk=False):\n",
                    "    return SimpleNamespace(exchange='0xexchange')\n",
                ),
            ),
            (
                "py_clob_client/signer.py",
                concat!(
                    "class Signer:\n",
                    "    def __init__(self, private_key, chain_id=137):\n",
                    "        self.private_key = private_key\n",
                    "        self.chain_id = chain_id\n",
                    "    def address(self):\n",
                    "        return '0xpoly-address'\n",
                ),
            ),
            (
                "py_clob_client/clob_types.py",
                concat!(
                    "from dataclasses import dataclass\n",
                    "@dataclass\n",
                    "class ApiCreds:\n",
                    "    api_key: str\n",
                    "    api_secret: str\n",
                    "    api_passphrase: str\n",
                    "@dataclass\n",
                    "class RequestArgs:\n",
                    "    method: str\n",
                    "    request_path: str\n",
                    "    body: str\n",
                ),
            ),
            ("py_clob_client/headers/__init__.py", ""),
            (
                "py_clob_client/headers/headers.py",
                concat!(
                    "def create_level_2_headers(signer, creds, request_args):\n",
                    "    return {\n",
                    "        'POLY_ADDRESS': signer.address(),\n",
                    "        'POLY_API_KEY': creds.api_key,\n",
                    "        'POLY_PASSPHRASE': creds.api_passphrase,\n",
                    "        'POLY_SIGNATURE': 'l2sig:POST:/orders',\n",
                    "        'POLY_TIMESTAMP': '1712345678',\n",
                    "    }\n",
                ),
            ),
            ("py_order_utils/__init__.py", ""),
            (
                "py_order_utils/model.py",
                concat!(
                    "from dataclasses import dataclass\n",
                    "@dataclass\n",
                    "class OrderData:\n",
                    "    maker: str\n",
                    "    taker: str\n",
                    "    tokenId: int\n",
                    "    makerAmount: int\n",
                    "    takerAmount: int\n",
                    "    side: str\n",
                    "    feeRateBps: int\n",
                    "    nonce: int\n",
                    "    signer: str\n",
                    "    expiration: int\n",
                    "    signatureType: int\n",
                ),
            ),
            ("py_order_utils/builders/__init__.py", ""),
            (
                "py_order_utils/builders/order_builder.py",
                concat!(
                    "class SignedOrder:\n",
                    "    def __init__(self, data):\n",
                    "        self.signature = 'ordersig:12345:2'\n",
                    "        self.salt = '7'\n",
                    "        self.maker = data.maker\n",
                    "        self.signer = data.signer\n",
                    "    def dict(self):\n",
                    "        return {\n",
                    "            'signature': self.signature,\n",
                    "            'salt': self.salt,\n",
                    "            'maker': self.maker,\n",
                    "            'signer': self.signer,\n",
                    "        }\n",
                    "class OrderBuilder:\n",
                    "    def __init__(self, signer, sig_type, funder, contract_config):\n",
                    "        self.signer = signer\n",
                    "    def build_signed_order(self, order_data):\n",
                    "        return SignedOrder(order_data)\n",
                ),
            ),
        ];

        for (relative, contents) in files {
            let path = stub_root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("stub parent created");
            }
            fs::write(path, contents).expect("stub file written");
        }

        let wrapper = root.join("python-with-stubs.sh");
        fs::write(
            &wrapper,
            format!(
                "#!/usr/bin/env bash\nexport PYTHONPATH=\"{}\"\nexec python3 \"$@\"\n",
                stub_root.display()
            ),
        )
        .expect("wrapper written");
        let mut perms = fs::metadata(&wrapper)
            .expect("wrapper metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper, perms).expect("wrapper perms");
        wrapper
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
        assert!(report.contains("l2_header_helper=disabled"));
        assert!(report.contains("submit_command=disabled"));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn bootstrap_report_loads_repo_local_helper_wiring_without_unlocking_live_mode() {
        let root = unique_temp_root("bootstrap-report-wired");
        fs::create_dir_all(&root).expect("temp root created");
        fs::write(
            root.join(".env.local"),
            concat!(
                "RUST_COPYTRADER_SIGNING_PROGRAM=python3\n",
                "RUST_COPYTRADER_SUBMIT_PROGRAM=curl\n",
                "CLOB_HOST=https://clob.polymarket.com\n",
                "RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS=75\n",
                "RUST_COPYTRADER_SUBMIT_MAX_TIME_MS=150\n",
            ),
        )
        .expect(".env.local written");

        let report = render_live_bootstrap_report(&root).expect("report should render");

        assert!(
            report.contains("selected_leader_wallet=0x56687bf447db6ffa42ffe2204a05edaa20f55839")
        );
        assert!(report.contains("selected_leader_source=fallback"));
        assert!(report.contains("decision=blocked:activity_source_unverified"));
        assert!(report.contains("live_mode_unlocked=false"));
        assert!(report.contains("signing_command=python3 scripts/sign_order.py --json"));
        assert!(report.contains("l2_header_helper=python3 scripts/sign_l2.py --json"));
        assert!(report.contains("submit_command=curl"));
        assert!(report.contains("submit_base_url=https://clob.polymarket.com"));
        assert!(report.contains("submit_connect_timeout_ms=75"));
        assert!(report.contains("submit_max_time_ms=150"));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn bootstrap_report_loads_submit_args_from_root() {
        let root = unique_temp_root("bootstrap-report-submit-args");
        fs::create_dir_all(&root).expect("temp root created");
        fs::write(
            root.join(".env.local"),
            concat!(
                "RUST_COPYTRADER_SIGNING_PROGRAM=python3\n",
                "RUST_COPYTRADER_SUBMIT_PROGRAM=python3\n",
                "RUST_COPYTRADER_SUBMIT_ARGS=scripts/submit_helper.py --json --curl-bin curl\n",
                "CLOB_HOST=https://clob.polymarket.com\n",
            ),
        )
        .expect(".env.local written");

        let report = render_live_bootstrap_report(&root).expect("report should render");

        assert!(
            report
                .contains("submit_command=python3 scripts/submit_helper.py --json --curl-bin curl")
        );

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn helper_smoke_report_stays_disabled_without_helper_wiring() {
        let root = unique_temp_root("helper-smoke-default");
        fs::create_dir_all(&root).expect("temp root created");

        let report = render_live_helper_smoke_report(&root).expect("report should render");

        assert!(report.contains("mode=helper-smoke"));
        assert!(report.contains("helper_smoke=disabled"));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn parse_root_arg_accepts_smoke_flag_before_root() {
        let args = vec![
            std::ffi::OsString::from("--smoke-helper"),
            std::ffi::OsString::from("--root"),
            std::ffi::OsString::from("/tmp/demo"),
        ];

        let root = super::parse_root_arg(&args).expect("root should parse");

        assert_eq!(root, PathBuf::from("/tmp/demo"));
    }

    #[test]
    fn parse_root_arg_accepts_runtime_smoke_flag_before_root() {
        let args = vec![
            std::ffi::OsString::from("--smoke-runtime"),
            std::ffi::OsString::from("--root"),
            std::ffi::OsString::from("/tmp/demo"),
        ];

        let root = super::parse_root_arg(&args).expect("root should parse");

        assert_eq!(root, PathBuf::from("/tmp/demo"));
    }

    #[test]
    fn parse_root_arg_accepts_operator_demo_flag_before_root() {
        let args = vec![
            std::ffi::OsString::from("--operator-demo"),
            std::ffi::OsString::from("--root"),
            std::ffi::OsString::from("/tmp/demo"),
        ];

        let root = super::parse_root_arg(&args).expect("root should parse");

        assert_eq!(root, PathBuf::from("/tmp/demo"));
    }

    #[test]
    fn helper_smoke_report_executes_repo_local_helpers_via_wrapper() {
        let root = unique_temp_root("helper-smoke-wired");
        fs::create_dir_all(&root).expect("temp root created");
        let wrapper = write_stub_sdk(&root);
        fs::write(
            root.join(".env.local"),
            format!(
                concat!(
                    "POLY_ADDRESS=0xpoly-address\n",
                    "CLOB_API_KEY=api-key\n",
                    "CLOB_SECRET=api-secret\n",
                    "CLOB_PASS_PHRASE=passphrase\n",
                    "PRIVATE_KEY=private-key\n",
                    "SIGNATURE_TYPE=2\n",
                    "FUNDER_ADDRESS=0xfunder-address\n",
                    "RUST_COPYTRADER_SIGNING_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_ARGS=scripts/submit_helper.py --json --curl-bin curl\n",
                    "CLOB_HOST=https://clob.polymarket.com\n",
                ),
                wrapper.display(),
                wrapper.display(),
            ),
        )
        .expect(".env.local written");

        let report = render_live_helper_smoke_report(&root).expect("helper smoke should render");

        assert!(report.contains("helper_smoke=ok"));
        assert!(report.contains("order_signature=ordersig:12345:2"));
        assert!(report.contains("order_salt=7"));
        assert!(report.contains("l2_signature=l2sig:POST:/orders"));
        assert!(report.contains("submit_preview_program="));
        assert!(report.contains("scripts/submit_helper.py --json --curl-bin curl"));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn runtime_smoke_report_persists_operator_artifacts() {
        let root = unique_temp_root("runtime-smoke-wired");
        fs::create_dir_all(&root).expect("temp root created");
        let wrapper = write_stub_sdk(&root);
        fs::write(
            root.join(".env.local"),
            format!(
                concat!(
                    "POLY_ADDRESS=0xpoly-address\n",
                    "CLOB_API_KEY=api-key\n",
                    "CLOB_SECRET=api-secret\n",
                    "CLOB_PASS_PHRASE=passphrase\n",
                    "PRIVATE_KEY=private-key\n",
                    "SIGNATURE_TYPE=2\n",
                    "FUNDER_ADDRESS=0xfunder-address\n",
                    "RUST_COPYTRADER_SIGNING_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_ARGS=scripts/submit_helper.py --json --curl-bin curl\n",
                    "CLOB_HOST=https://clob.polymarket.com\n",
                ),
                wrapper.display(),
                wrapper.display(),
            ),
        )
        .expect(".env.local written");

        let report = render_runtime_smoke_report(&root).expect("runtime smoke should render");

        assert!(report.contains("mode=runtime-smoke"));
        assert!(report.contains("helper_smoke=ok"));
        assert!(report.contains("session_outcome=processed"));
        assert!(report.contains("replay_submit_elapsed_ms=60"));
        assert!(report.contains("replay_verified_elapsed_ms=82"));
        assert!(report.contains("submit_hard_budget_ms=200"));
        assert!(report.contains("submit_budget_headroom_ms=140"));
        assert!(report.contains("runtime_mode=replay"));
        assert!(report.contains("last_submit_status=verified"));
        assert!(report.contains("latest_snapshot_path="));
        assert!(report.contains("report_path="));
        assert!(report.contains("summary_path="));

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn operator_demo_report_includes_discovery_hints() {
        let root = unique_temp_root("operator-demo-wired");
        fs::create_dir_all(&root).expect("temp root created");
        let wrapper = write_stub_sdk(&root);
        fs::write(
            root.join(".env.local"),
            format!(
                concat!(
                    "POLY_ADDRESS=0xpoly-address\n",
                    "CLOB_API_KEY=api-key\n",
                    "CLOB_SECRET=api-secret\n",
                    "CLOB_PASS_PHRASE=passphrase\n",
                    "PRIVATE_KEY=private-key\n",
                    "SIGNATURE_TYPE=2\n",
                    "FUNDER_ADDRESS=0xfunder-address\n",
                    "RUST_COPYTRADER_SIGNING_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_PROGRAM={}\n",
                    "RUST_COPYTRADER_SUBMIT_ARGS=scripts/submit_helper.py --json --curl-bin curl\n",
                    "CLOB_HOST=https://clob.polymarket.com\n",
                ),
                wrapper.display(),
                wrapper.display(),
            ),
        )
        .expect(".env.local written");

        let report = render_operator_demo_report(&root).expect("operator demo should render");

        assert!(report.contains("mode=operator-demo"));
        assert!(report.contains("mode=runtime-smoke"));
        assert!(report.contains("selected_leader_wallet=0xpoly-address"));
        assert!(report.contains("selected_leader_source=auth_material"));
        assert!(report.contains("leaderboard_hint="));
        assert!(report.contains("activity_hint=cd rust-copytrader && cargo run --bin fetch_user_activity -- --user 0xpoly-address --type TRADE --limit 20"));
        assert!(report.contains("leaderboard_preview_url=https://data-api.polymarket.com/v1/leaderboard?category=OVERALL&timePeriod=DAY&orderBy=PNL&limit=20&offset=0"));
        assert!(report.contains("leaderboard_preview_curl=curl --silent --show-error --fail-with-body -A Mozilla/5.0 -H 'Accept: application/json' https://data-api.polymarket.com/v1/leaderboard?category=OVERALL&timePeriod=DAY&orderBy=PNL&limit=20&offset=0"));
        assert!(report.contains("activity_preview_url=https://data-api.polymarket.com/activity?user=0xpoly-address&limit=20&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type=TRADE"));
        assert!(report.contains("activity_preview_curl=curl --silent --show-error --fail-with-body -A Mozilla/5.0 -H 'Accept: application/json' https://data-api.polymarket.com/activity?user=0xpoly-address&limit=20&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type=TRADE"));
        assert!(report.contains("leaderboard_capture_hint=cd rust-copytrader && cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --output ../.omx/discovery/leaderboard-overall-day-pnl.json"));
        assert!(report.contains("activity_capture_hint=cd rust-copytrader && cargo run --bin fetch_user_activity -- --user 0xpoly-address --type TRADE --limit 20 --output ../.omx/discovery/activity-0xpoly-address-trade.json"));
        assert!(report.contains("leader_selection_hint=cd rust-copytrader && cargo run --bin select_copy_leader -- --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env"));
        assert!(report.contains("leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a"));
        assert!(report.contains("note=public discovery commands are read-only"));
        let report_path = report
            .lines()
            .find_map(|line| line.strip_prefix("operator_demo_report_path="))
            .expect("report path line");
        let latest_path = report
            .lines()
            .find_map(|line| line.strip_prefix("operator_demo_latest_path="))
            .expect("latest path line");
        let persisted = fs::read_to_string(report_path).expect("operator demo report persisted");
        let latest = fs::read_to_string(latest_path).expect("operator demo latest persisted");
        assert!(persisted.contains("mode=operator-demo"));
        assert!(persisted.contains("leaderboard_hint="));
        assert_eq!(persisted, latest);

        fs::remove_dir_all(root).expect("temp root removed");
    }

    #[test]
    fn operator_demo_prefers_selected_leader_env_file_when_present() {
        let root = unique_temp_root("operator-demo-selected-leader");
        fs::create_dir_all(root.join(".omx/discovery")).expect("discovery dir created");
        fs::write(
            root.join(".omx/discovery/selected-leader.env"),
            "COPYTRADER_DISCOVERY_WALLET=0xselected-wallet\n",
        )
        .expect("selected leader env written");

        let (selected_wallet, selected_source) = super::selected_leader_context(&root);

        assert_eq!(selected_wallet, "0xselected-wallet");
        assert_eq!(selected_source, "file:.omx/discovery/selected-leader.env");

        fs::remove_dir_all(root).expect("temp root removed");
    }
}
