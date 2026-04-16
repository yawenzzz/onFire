use rust_copytrader::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use rust_copytrader::adapters::http_submit::{
    CommandOutput, CommandRunner, CurlCommandSpec, HttpRequestSpec, HttpSubmitBuildError,
    HttpSubmitClientConfig, HttpSubmitCommandError, HttpSubmitExecutor, HttpSubmitLiveError,
    HttpSubmitResponseProtocolError, HttpSubmitTransportError, HttpSubmitter, OrderBatchRequest,
    OrderType, SignedOrderEnvelope, SignedOrderPayload,
};
use rust_copytrader::config::{
    CommandAdapterConfig, ExecutionAdapterConfig, LiveExecutionWiring, SubmitAdapterConfig,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn sample_request_spec() -> HttpRequestSpec {
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let builder = rust_copytrader::adapters::http_submit::HttpSubmitRequestBuilder::new(
        "https://clob.polymarket.com",
    );
    let payload = SignedOrderPayload {
        maker: "0xmaker".into(),
        signer: "0xsigner".into(),
        taker: "0x0000000000000000000000000000000000000000".into(),
        token_id: "12345".into(),
        maker_amount: "1000000".into(),
        taker_amount: "2000000".into(),
        side: "BUY".into(),
        expiration: "1735689600".into(),
        nonce: "7".into(),
        fee_rate_bps: "30".into(),
        signature_type: 0,
        signature: "0xsig".into(),
        salt: "999".into(),
    };
    let batch = OrderBatchRequest::single(SignedOrderEnvelope::new(
        payload,
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));
    builder
        .build(&auth, &headers, &batch)
        .expect("request spec")
}

fn sample_batch() -> OrderBatchRequest {
    OrderBatchRequest::single(SignedOrderEnvelope::new(
        SignedOrderPayload {
            maker: "0xmaker".into(),
            signer: "0xsigner".into(),
            taker: "0x0000000000000000000000000000000000000000".into(),
            token_id: "12345".into(),
            maker_amount: "1000000".into(),
            taker_amount: "2000000".into(),
            side: "BUY".into(),
            expiration: "1735689600".into(),
            nonce: "7".into(),
            fee_rate_bps: "30".into(),
            signature_type: 0,
            signature: "0xsig".into(),
            salt: "999".into(),
        },
        "owner-uuid",
        OrderType::Gtc,
        false,
    ))
}

fn unique_temp_root(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nanos))
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has repo root parent")
}

#[test]
fn curl_command_contains_method_url_headers_and_body() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::from_config(
        HttpSubmitClientConfig::new("curl")
            .with_connect_timeout_ms(75)
            .with_max_time_ms(150),
    );

    let command = executor.build_command(&spec);

    assert_eq!(command.program, "curl");
    assert!(command.args.windows(2).any(|pair| pair == ["-X", "POST"]));
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg == "https://clob.polymarket.com/orders")
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("POLY_API_KEY: api-key"))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("POLY_SIGNATURE: signature"))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("Content-Type: application/json"))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"owner\":\"owner-uuid\""))
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--connect-timeout", "0.075"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--max-time", "0.150"])
    );
    assert!(command.args.iter().any(|arg| arg == "--fail-with-body"));
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--write-out", "\n__HTTP_STATUS__:%{http_code}"])
    );
}

#[test]
fn executor_returns_parsed_http_body_and_status_on_success() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:201");

    let result = executor
        .execute(&mut runner, &spec)
        .expect("execution result");

    assert_eq!(result.status_code, 201);
    assert_eq!(result.body, "{\"ok\":true}");
    assert_eq!(runner.calls, 1);
}

#[test]
fn executor_rejects_non_success_http_status_even_when_command_exits_zero() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");
    let mut runner = StubRunner::success("{\"error\":\"blocked\"}\n__HTTP_STATUS__:401");

    let err = executor.execute(&mut runner, &spec).unwrap_err();

    assert_eq!(
        err,
        HttpSubmitCommandError::HttpStatus {
            status_code: 401,
            body: "{\"error\":\"blocked\"}".into(),
        }
    );
}

#[test]
fn executor_reclassifies_non_zero_http_status_with_response_body() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");
    let mut runner = StubRunner::failure_with_stdout(
        22,
        "{\"error\":\"blocked\"}\n__HTTP_STATUS__:401",
        "curl: (22) The requested URL returned error: 401",
    );

    let err = executor.execute(&mut runner, &spec).unwrap_err();

    assert_eq!(
        err,
        HttpSubmitCommandError::HttpStatus {
            status_code: 401,
            body: "{\"error\":\"blocked\"}".into(),
        }
    );
}

#[test]
fn executor_rejects_success_output_without_http_status_marker() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");
    let mut runner = StubRunner::success("{\"ok\":true}");

    let err = executor.execute(&mut runner, &spec).unwrap_err();

    assert_eq!(err, HttpSubmitCommandError::MissingStatusMarker);
}

#[test]
fn executor_surfaces_non_zero_exit_with_stderr() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");
    let mut runner = StubRunner::failure(28, "operation timed out");

    let err = executor.execute(&mut runner, &spec).unwrap_err();

    assert_eq!(
        err,
        HttpSubmitCommandError::NonZeroExit {
            code: 28,
            stderr: "operation timed out".into(),
        }
    );
}

#[test]
fn live_submitter_executes_and_returns_request_and_response() {
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "0xheader-sig",
        "1712345678",
    );
    let batch = OrderBatchRequest::single(SignedOrderEnvelope::new(
        SignedOrderPayload {
            maker: "0xmaker".into(),
            signer: "0xsigner".into(),
            taker: "0x0000000000000000000000000000000000000000".into(),
            token_id: "12345".into(),
            maker_amount: "1000000".into(),
            taker_amount: "2000000".into(),
            side: "BUY".into(),
            expiration: "1735689600".into(),
            nonce: "7".into(),
            fee_rate_bps: "30".into(),
            signature_type: 0,
            signature: "0xsig".into(),
            salt: "999".into(),
        },
        "owner-uuid",
        OrderType::Fak,
        true,
    ));
    let submitter = HttpSubmitter::new("https://clob.polymarket.com", "curl");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:201");

    let result = submitter
        .submit(&auth, &headers, &batch, &mut runner)
        .expect("live submit result");

    assert_eq!(result.response.status_code, 201);
    assert_eq!(result.response.body, "{\"ok\":true}");
    assert_eq!(result.request.url, "https://clob.polymarket.com/orders");
    assert_eq!(
        result
            .request
            .headers
            .get("POLY_SIGNATURE")
            .map(String::as_str),
        Some("0xheader-sig")
    );
    assert!(result.request.body.contains("\"orderType\":\"FAK\""));
    assert!(result.request.body.contains("\"deferExec\":true"));
    assert_eq!(runner.calls, 1);
}

#[test]
fn live_submitter_classifies_http_and_transport_failures_strictly() {
    let submitter = HttpSubmitter::from_parts(
        rust_copytrader::adapters::http_submit::HttpSubmitRequestBuilder::new(
            "https://clob.polymarket.com",
        ),
        HttpSubmitExecutor::new("curl"),
    );
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let batch = OrderBatchRequest::single(SignedOrderEnvelope::new(
        SignedOrderPayload {
            maker: "0xmaker".into(),
            signer: "0xsigner".into(),
            taker: "0x0000000000000000000000000000000000000000".into(),
            token_id: "12345".into(),
            maker_amount: "1000000".into(),
            taker_amount: "2000000".into(),
            side: "BUY".into(),
            expiration: "1735689600".into(),
            nonce: "7".into(),
            fee_rate_bps: "30".into(),
            signature_type: 0,
            signature: "0xsig".into(),
            salt: "999".into(),
        },
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));
    let mut http_rejection_runner = StubRunner::failure_with_stdout(
        22,
        "{\"error\":\"blocked\"}\n__HTTP_STATUS__:401",
        "curl: (22) The requested URL returned error: 401",
    );

    let http_err = submitter
        .submit(&auth, &headers, &batch, &mut http_rejection_runner)
        .unwrap_err();

    assert_eq!(
        http_err,
        HttpSubmitLiveError::HttpStatus {
            status_code: 401,
            body: "{\"error\":\"blocked\"}".into(),
        }
    );

    let mut malformed_runner = StubRunner::success("{\"ok\":true}");
    let malformed_err = submitter
        .submit(&auth, &headers, &batch, &mut malformed_runner)
        .unwrap_err();

    assert_eq!(
        malformed_err,
        HttpSubmitLiveError::ResponseProtocol(HttpSubmitResponseProtocolError::MissingStatusMarker)
    );

    let mut transport_runner = StubRunner::failure(28, "operation timed out");
    let transport_err = submitter
        .submit(&auth, &headers, &batch, &mut transport_runner)
        .unwrap_err();

    assert_eq!(
        transport_err,
        HttpSubmitLiveError::Transport(HttpSubmitTransportError::CommandExit {
            code: 28,
            stderr: "operation timed out".into(),
        })
    );
}

#[test]
fn live_submitter_builder_uses_live_execution_wiring_timeouts() {
    let submitter = HttpSubmitter::from_live_execution_wiring(&LiveExecutionWiring {
        signing: CommandAdapterConfig::new("python3"),
        submit: CommandAdapterConfig::new("curl"),
        submit_base_url: "https://clob.polymarket.com".into(),
        submit_connect_timeout_ms: 75,
        submit_max_time_ms: 150,
    })
    .expect("submitter from live wiring");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let batch = OrderBatchRequest::single(SignedOrderEnvelope::new(
        SignedOrderPayload {
            maker: "0xmaker".into(),
            signer: "0xsigner".into(),
            taker: "0x0000000000000000000000000000000000000000".into(),
            token_id: "12345".into(),
            maker_amount: "1000000".into(),
            taker_amount: "2000000".into(),
            side: "BUY".into(),
            expiration: "1735689600".into(),
            nonce: "7".into(),
            fee_rate_bps: "30".into(),
            signature_type: 0,
            signature: "0xsig".into(),
            salt: "999".into(),
        },
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");

    submitter
        .submit(&auth, &headers, &batch, &mut runner)
        .expect("submit succeeds");

    let command = runner.last_command.expect("command captured");
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--connect-timeout", "0.075"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--max-time", "0.150"])
    );
}

#[test]
fn live_submitter_builder_fails_closed_when_submit_program_is_missing() {
    let err = HttpSubmitter::from_live_execution_wiring(&LiveExecutionWiring {
        signing: CommandAdapterConfig::new("python3"),
        submit: CommandAdapterConfig::new(""),
        submit_base_url: "https://clob.polymarket.com".into(),
        submit_connect_timeout_ms: 75,
        submit_max_time_ms: 150,
    })
    .unwrap_err();

    assert_eq!(err, HttpSubmitBuildError::MissingCommandProgram);
}

#[test]
fn live_submitter_uses_repo_loaded_submit_config_without_hand_built_live_wiring() {
    let root = unique_temp_root("http-submit-root");
    fs::create_dir_all(&root).expect("temp root created");
    fs::write(
        root.join(".env.local"),
        concat!(
            "RUST_COPYTRADER_SIGNING_PROGRAM=python3\n",
            "RUST_COPYTRADER_SUBMIT_PROGRAM=curl\n",
            "CLOB_HOST=https://helper.polymarket.test/\n",
            "RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS=75\n",
            "RUST_COPYTRADER_SUBMIT_MAX_TIME_MS=150\n",
        ),
    )
    .expect(".env.local written");
    let submitter = HttpSubmitter::from_root(&root).expect("submitter from root");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");

    submitter
        .submit(&auth, &headers, &sample_batch(), &mut runner)
        .expect("submit succeeds");

    let command = runner.last_command.expect("command captured");
    assert_eq!(command.program, "curl");
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg == "https://helper.polymarket.test/orders")
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--connect-timeout", "0.075"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--max-time", "0.150"])
    );

    fs::remove_dir_all(root).expect("temp root removed");
}

#[test]
fn live_submitter_preserves_repo_local_helper_args_from_submit_config() {
    let execution_config = ExecutionAdapterConfig {
        signing: rust_copytrader::config::SigningAdapterConfig::command("python3"),
        submit: SubmitAdapterConfig::http_with_command(
            "https://helper.polymarket.test",
            CommandAdapterConfig::new("python3")
                .with_args(vec!["scripts/submit_helper.py".into(), "--json".into()]),
        )
        .with_connect_timeout_ms(75)
        .with_max_time_ms(150),
    };
    let submitter = HttpSubmitter::from_submit_adapter_config(&execution_config.submit)
        .expect("submitter from submit config");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");

    submitter
        .submit(&auth, &headers, &sample_batch(), &mut runner)
        .expect("submit succeeds");

    let command = runner.last_command.expect("command captured");
    assert_eq!(command.program, "python3");
    assert_eq!(command.args[0], "scripts/submit_helper.py");
    assert_eq!(command.args[1], "--json");
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg == "https://helper.polymarket.test/orders")
    );
}

#[test]
fn repo_local_submit_helper_executes_real_wrapper_and_forwards_http_submit_args() {
    let root = unique_temp_root("http-submit-helper-wrapper");
    fs::create_dir_all(&root).expect("temp root created");
    let log_path = root.join("curl-args.log");
    let stub_curl = root.join("stub_curl.py");
    fs::write(
        &stub_curl,
        format!(
            concat!(
                "#!/usr/bin/env python3\n",
                "import pathlib\n",
                "import sys\n",
                "pathlib.Path({log_path:?}).write_text('\\n'.join(sys.argv[1:]), encoding='utf-8')\n",
                "sys.stdout.write('{{\"ok\":true}}\\n__HTTP_STATUS__:200')\n",
            ),
            log_path = log_path.display().to_string(),
        ),
    )
    .expect("stub curl written");
    #[cfg(unix)]
    fs::set_permissions(&stub_curl, fs::Permissions::from_mode(0o755))
        .expect("stub curl executable");
    let helper_path = repo_root().join("scripts/submit_helper.py");
    let submitter =
        HttpSubmitter::from_submit_adapter_config(&SubmitAdapterConfig::http_with_command(
            "https://helper.polymarket.test",
            CommandAdapterConfig::new("python3").with_args(vec![
                helper_path.display().to_string(),
                "--json".into(),
                "--curl-bin".into(),
                stub_curl.display().to_string(),
            ]),
        ))
        .expect("submitter from helper config");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let mut runner = rust_copytrader::adapters::http_submit::StdCommandRunner;

    let result = submitter
        .submit(&auth, &headers, &sample_batch(), &mut runner)
        .expect("submit succeeds via wrapper");

    assert_eq!(result.response.status_code, 200);
    assert_eq!(result.response.body, "{\"ok\":true}");
    let forwarded = fs::read_to_string(&log_path).expect("forwarded args logged");
    assert!(forwarded.contains("--silent"));
    assert!(forwarded.contains("--show-error"));
    assert!(forwarded.contains("https://helper.polymarket.test/orders"));
    assert!(forwarded.contains("POLY_SIGNATURE: signature"));
    assert!(forwarded.contains("\"owner\":\"owner-uuid\""));

    fs::remove_dir_all(root).expect("temp root removed");
}

struct StubRunner {
    result: Result<CommandOutput, HttpSubmitCommandError>,
    calls: usize,
    last_command: Option<CurlCommandSpec>,
}

impl StubRunner {
    fn success(stdout: &str) -> Self {
        Self {
            result: Ok(CommandOutput {
                exit_code: 0,
                stdout: stdout.into(),
                stderr: String::new(),
            }),
            calls: 0,
            last_command: None,
        }
    }

    fn failure(code: i32, stderr: &str) -> Self {
        Self {
            result: Err(HttpSubmitCommandError::NonZeroExit {
                code,
                stderr: stderr.into(),
            }),
            calls: 0,
            last_command: None,
        }
    }

    fn failure_with_stdout(code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            result: Err(HttpSubmitCommandError::NonZeroExitWithOutput {
                code,
                stdout: stdout.into(),
                stderr: stderr.into(),
            }),
            calls: 0,
            last_command: None,
        }
    }
}

impl CommandRunner for StubRunner {
    fn run(&mut self, command: &CurlCommandSpec) -> Result<CommandOutput, HttpSubmitCommandError> {
        self.calls += 1;
        self.last_command = Some(command.clone());
        self.result.clone()
    }
}
