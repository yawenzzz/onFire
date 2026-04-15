use rust_copytrader::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use rust_copytrader::adapters::http_submit::{
    CommandOutput, CommandRunner, CurlCommandSpec, HttpRequestSpec, HttpSubmitCommandError,
    HttpSubmitExecutor, OrderBatchRequest, OrderType, SignedOrderEnvelope, SignedOrderPayload,
};

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

#[test]
fn curl_command_contains_method_url_headers_and_body() {
    let spec = sample_request_spec();
    let executor = HttpSubmitExecutor::new("curl");

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
            .any(|pair| pair == ["--max-time", "0.200"])
    );
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

struct StubRunner {
    result: Result<CommandOutput, HttpSubmitCommandError>,
    calls: usize,
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
        }
    }

    fn failure(code: i32, stderr: &str) -> Self {
        Self {
            result: Err(HttpSubmitCommandError::NonZeroExit {
                code,
                stderr: stderr.into(),
            }),
            calls: 0,
        }
    }
}

impl CommandRunner for StubRunner {
    fn run(&mut self, _command: &CurlCommandSpec) -> Result<CommandOutput, HttpSubmitCommandError> {
        self.calls += 1;
        self.result.clone()
    }
}
