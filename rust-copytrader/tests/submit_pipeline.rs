use rust_copytrader::adapters::auth::L2AuthHeaders;
use rust_copytrader::adapters::http_submit::{
    CommandOutput, CommandRunner, CurlCommandSpec, HttpSubmitCommandError, OrderType,
};
use rust_copytrader::adapters::signing::{
    AuthMaterial, CommandOrderSigner, OrderSigner, SigningArtifacts, SigningCommandError,
    SigningCommandOutput, SigningCommandRunner, SigningCommandSpec, SigningError,
    UnsignedOrderPayload,
};
use rust_copytrader::adapters::submit_pipeline::{
    L2HeaderProvider, PreparedSubmitRequest, SubmitPipeline, SubmitPipelineError,
};
use rust_copytrader::config::{CommandAdapterConfig, LiveExecutionWiring};

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

#[test]
fn pipeline_rejects_material_that_is_not_submit_ready() {
    let material = AuthMaterial::new("0xpoly-address", "api-key", "passphrase", "", 0, None);
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let err = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .unwrap_err();

    assert_eq!(
        err,
        SubmitPipelineError::AuthMaterial(
            rust_copytrader::adapters::signing::AuthMaterialError::MissingField(
                "PRIVATE_KEY".into()
            )
        )
    );
    assert_eq!(header_provider.calls, 0);
}

#[test]
fn pipeline_executes_end_to_end_and_preserves_request_shape() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let output = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Fak,
                defer_exec: true,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .expect("successful output");

    assert_eq!(output.status_code, 200);
    assert_eq!(output.body, "{\"ok\":true}");
    assert_eq!(signer.calls, 1);
    assert_eq!(header_provider.calls, 1);
    assert_eq!(runner.calls, 1);

    let command = runner.last_command.expect("command captured");
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
            .any(|arg| arg.contains("POLY_SIGNATURE: 0xheader-sig"))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"signature\":\"0xorder-sig\""))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"orderType\":\"FAK\""))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"deferExec\":true"))
    );
}

#[test]
fn pipeline_surfaces_signer_failure_before_running_command() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = StubSigner::failure("unable_to_sign");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let err = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .unwrap_err();

    assert_eq!(
        err,
        SubmitPipelineError::Signing(SigningError::Signer("unable_to_sign".into()))
    );
    assert_eq!(header_provider.calls, 0);
    assert_eq!(runner.calls, 0);
}

#[test]
fn pipeline_surfaces_runner_failure_after_request_construction() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::failure(28, "operation timed out");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let err = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .unwrap_err();

    assert_eq!(
        err,
        SubmitPipelineError::Command(HttpSubmitCommandError::NonZeroExit {
            code: 28,
            stderr: "operation timed out".into(),
        })
    );
    assert_eq!(header_provider.calls, 1);
    assert_eq!(runner.calls, 1);
}

#[test]
fn pipeline_uses_funder_as_order_maker_for_proxy_wallet_flow() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        2,
        Some("0xfunder-address".into()),
    );
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .expect("successful output");

    let command = runner.last_command.expect("command captured");
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"maker\":\"0xfunder-address\""))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("\"signer\":\"0xpoly-address\""))
    );
    assert!(
        command
            .args
            .iter()
            .any(|arg| arg.contains("POLY_ADDRESS: 0xpoly-address"))
    );
}

#[test]
fn pipeline_surfaces_header_provider_failure_before_running_submit_command() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::failure("header_signing_failed");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let err = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .unwrap_err();

    assert_eq!(
        err,
        SubmitPipelineError::HeaderSigning(SigningError::Signer("header_signing_failed".into()))
    );
    assert_eq!(signer.calls, 1);
    assert_eq!(header_provider.calls, 1);
    assert_eq!(runner.calls, 0);
}

#[test]
fn pipeline_accepts_command_backed_signer_and_preserves_signed_output() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        2,
        Some("0xfunder-address".into()),
    );
    let signing_runner =
        StubSigningCommandRunner::success("{\"signature\":\"0xcmd-order-sig\",\"salt\":\"777\"}");
    let mut signer = CommandOrderSigner::new(
        "python3",
        vec!["scripts/sign_order.py".into(), "--json".into()],
        signing_runner,
    );
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::new("https://clob.polymarket.com", "curl");

    let output = pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
        .expect("successful output");

    assert_eq!(output.status_code, 200);
    let signing_command = signer
        .runner()
        .last_command
        .as_ref()
        .expect("signing command captured");
    assert_eq!(
        signing_command
            .env
            .get("CLOB_PRIVATE_KEY")
            .map(String::as_str),
        Some("private-key")
    );
    assert_eq!(
        signing_command.env.get("FUNDER").map(String::as_str),
        Some("0xfunder-address")
    );
    assert!(signing_command.stdin.contains("\"feeRateBps\":\"30\""));

    let submit_command = runner.last_command.expect("submit command captured");
    assert!(
        submit_command
            .args
            .iter()
            .any(|arg| arg.contains("\"signature\":\"0xcmd-order-sig\""))
    );
    assert!(
        submit_command
            .args
            .iter()
            .any(|arg| arg.contains("\"salt\":\"777\""))
    );
    assert_eq!(header_provider.calls, 1);
}

#[test]
fn pipeline_builder_uses_live_execution_wiring_timeouts_for_helper_submit_path() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = StubSigner::success("0xorder-sig", "999");
    let mut header_provider = StubHeaderProvider::success("0xheader-sig", "1712345678");
    let mut runner = StubRunner::success("{\"ok\":true}\n__HTTP_STATUS__:200");
    let pipeline = SubmitPipeline::from_live_execution_wiring(&LiveExecutionWiring {
        signing: CommandAdapterConfig::repo_local_order_sign_helper("python3"),
        submit: CommandAdapterConfig::new("curl"),
        submit_base_url: "https://clob.polymarket.com".into(),
        submit_connect_timeout_ms: 75,
        submit_max_time_ms: 150,
    })
    .expect("pipeline from live helper wiring");

    pipeline
        .execute(
            PreparedSubmitRequest {
                auth_material: material,
                unsigned_order: sample_unsigned_order(),
                owner: "owner-uuid".into(),
                order_type: OrderType::Gtc,
                defer_exec: false,
                sdk_available: true,
            },
            &mut signer,
            &mut header_provider,
            &mut runner,
        )
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

struct StubSigner {
    result: Result<SigningArtifacts, SigningError>,
    calls: usize,
}

impl StubSigner {
    fn success(signature: &str, salt: &str) -> Self {
        Self {
            result: Ok(SigningArtifacts {
                signature: signature.into(),
                salt: salt.into(),
            }),
            calls: 0,
        }
    }

    fn failure(reason: &str) -> Self {
        Self {
            result: Err(SigningError::Signer(reason.into())),
            calls: 0,
        }
    }
}

impl OrderSigner for StubSigner {
    fn sign_order(
        &mut self,
        _payload: &UnsignedOrderPayload,
        _material: &AuthMaterial,
    ) -> Result<SigningArtifacts, SigningError> {
        self.calls += 1;
        self.result.clone()
    }
}

struct StubHeaderProvider {
    result: Result<L2AuthHeaders, SigningError>,
    calls: usize,
}

impl StubHeaderProvider {
    fn success(signature: &str, timestamp: &str) -> Self {
        Self {
            result: Ok(L2AuthHeaders::new(
                "0xpoly-address",
                "api-key",
                "passphrase",
                signature,
                timestamp,
            )),
            calls: 0,
        }
    }

    fn failure(reason: &str) -> Self {
        Self {
            result: Err(SigningError::Signer(reason.into())),
            calls: 0,
        }
    }
}

impl L2HeaderProvider for StubHeaderProvider {
    fn l2_headers(&mut self, _material: &AuthMaterial) -> Result<L2AuthHeaders, SigningError> {
        self.calls += 1;
        self.result.clone()
    }
}

struct StubSigningCommandRunner {
    result: Result<SigningCommandOutput, SigningCommandError>,
    last_command: Option<SigningCommandSpec>,
}

impl StubSigningCommandRunner {
    fn success(stdout: &str) -> Self {
        Self {
            result: Ok(SigningCommandOutput {
                exit_code: 0,
                stdout: stdout.into(),
                stderr: String::new(),
            }),
            last_command: None,
        }
    }
}

impl SigningCommandRunner for StubSigningCommandRunner {
    fn run(
        &mut self,
        command: &SigningCommandSpec,
    ) -> Result<SigningCommandOutput, SigningCommandError> {
        self.last_command = Some(command.clone());
        self.result.clone()
    }
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
}

impl CommandRunner for StubRunner {
    fn run(&mut self, command: &CurlCommandSpec) -> Result<CommandOutput, HttpSubmitCommandError> {
        self.calls += 1;
        self.last_command = Some(command.clone());
        self.result.clone()
    }
}
