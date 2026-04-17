use crate::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use crate::config::{
    CommandAdapterConfig, ExecutionAdapterConfig, LiveExecutionWiring, RootEnvLoadError,
    SubmitAdapterConfig,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const HTTP_STATUS_MARKER: &str = "\n__HTTP_STATUS__:";
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 50;
const DEFAULT_MAX_TIME_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestSpec {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Gtc,
    Gtd,
    Fok,
    Fak,
}

impl OrderType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gtc => "GTC",
            Self::Gtd => "GTD",
            Self::Fok => "FOK",
            Self::Fak => "FAK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOrderPayload {
    pub maker: String,
    pub signer: String,
    pub taker: String,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub side: String,
    pub expiration: String,
    pub nonce: String,
    pub fee_rate_bps: String,
    pub signature_type: u8,
    pub signature: String,
    pub salt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOrderEnvelope {
    pub order: SignedOrderPayload,
    pub owner: String,
    pub order_type: OrderType,
    pub defer_exec: bool,
}

impl SignedOrderEnvelope {
    pub fn new(
        order: SignedOrderPayload,
        owner: impl Into<String>,
        order_type: OrderType,
        defer_exec: bool,
    ) -> Self {
        Self {
            order,
            owner: owner.into(),
            order_type,
            defer_exec,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBatchRequest {
    pub orders: Vec<SignedOrderEnvelope>,
}

impl OrderBatchRequest {
    pub fn single(order: SignedOrderEnvelope) -> Self {
        Self {
            orders: vec![order],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitRequestError {
    ExecutionSurfaceNotReady(String),
    MissingHeader(String),
    EmptyBatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitRequestBuilder {
    base_url: String,
}

impl HttpSubmitRequestBuilder {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn from_submit_adapter_config(
        config: &SubmitAdapterConfig,
    ) -> Result<Self, HttpSubmitBuildError> {
        match config {
            SubmitAdapterConfig::Replay => Err(HttpSubmitBuildError::MissingBaseUrl),
            SubmitAdapterConfig::Http { base_url, .. } => {
                if base_url.trim().is_empty() {
                    return Err(HttpSubmitBuildError::MissingBaseUrl);
                }
                Ok(Self::new(base_url))
            }
        }
    }

    pub fn build(
        &self,
        auth: &AuthRuntimeState,
        headers: &L2AuthHeaders,
        batch: &OrderBatchRequest,
    ) -> Result<HttpRequestSpec, HttpSubmitRequestError> {
        if let Some(reason) = auth.blocked_reason() {
            return Err(HttpSubmitRequestError::ExecutionSurfaceNotReady(
                reason.to_string(),
            ));
        }
        if let Some(header) = headers.missing_header() {
            return Err(HttpSubmitRequestError::MissingHeader(header.to_string()));
        }
        if batch.orders.is_empty() {
            return Err(HttpSubmitRequestError::EmptyBatch);
        }

        let mut spec_headers = BTreeMap::new();
        spec_headers.insert("Accept".to_string(), "application/json".to_string());
        spec_headers.insert("Content-Type".to_string(), "application/json".to_string());
        spec_headers.insert("POLY_ADDRESS".to_string(), headers.poly_address.clone());
        spec_headers.insert("POLY_API_KEY".to_string(), headers.poly_api_key.clone());
        spec_headers.insert(
            "POLY_PASSPHRASE".to_string(),
            headers.poly_passphrase.clone(),
        );
        spec_headers.insert("POLY_SIGNATURE".to_string(), headers.poly_signature.clone());
        spec_headers.insert("POLY_TIMESTAMP".to_string(), headers.poly_timestamp.clone());

        Ok(HttpRequestSpec {
            method: HttpMethod::Post,
            url: format!("{}/orders", self.base_url),
            headers: spec_headers,
            body: render_batch_json(batch),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurlCommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitClientConfig {
    pub program: String,
    pub base_args: Vec<String>,
    pub connect_timeout_ms: u64,
    pub max_time_ms: u64,
    pub fail_on_http_error: bool,
}

impl HttpSubmitClientConfig {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            base_args: Vec::new(),
            connect_timeout_ms: DEFAULT_CONNECT_TIMEOUT_MS,
            max_time_ms: DEFAULT_MAX_TIME_MS,
            fail_on_http_error: true,
        }
    }

    pub fn from_command_config(command: &CommandAdapterConfig) -> Self {
        Self::new(command.program.clone()).with_base_args(command.args.clone())
    }

    pub fn with_base_args(mut self, base_args: Vec<String>) -> Self {
        self.base_args = base_args;
        self
    }

    pub fn with_connect_timeout_ms(mut self, connect_timeout_ms: u64) -> Self {
        self.connect_timeout_ms = connect_timeout_ms.max(1);
        self
    }

    pub fn with_max_time_ms(mut self, max_time_ms: u64) -> Self {
        self.max_time_ms = max_time_ms.max(1);
        self
    }

    pub fn with_fail_on_http_error(mut self, fail_on_http_error: bool) -> Self {
        self.fail_on_http_error = fail_on_http_error;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitResponse {
    pub status_code: u16,
    pub body: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitLiveResult {
    pub request: HttpRequestSpec,
    pub response: HttpSubmitResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitCommandError {
    Io(String),
    NonZeroExit {
        code: i32,
        stderr: String,
    },
    NonZeroExitWithOutput {
        code: i32,
        stdout: String,
        stderr: String,
    },
    MissingStatusMarker,
    InvalidStatusCode(String),
    HttpStatus {
        status_code: u16,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitTransportError {
    Io(String),
    CommandExit {
        code: i32,
        stderr: String,
    },
    CommandExitWithOutput {
        code: i32,
        stdout: String,
        stderr: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitResponseProtocolError {
    MissingStatusMarker,
    InvalidStatusCode(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitLiveError {
    Request(HttpSubmitRequestError),
    Transport(HttpSubmitTransportError),
    ResponseProtocol(HttpSubmitResponseProtocolError),
    HttpStatus { status_code: u16, body: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitBuildError {
    MissingBaseUrl,
    MissingCommandProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpSubmitRootBuildError {
    Config(RootEnvLoadError),
    Build(HttpSubmitBuildError),
}

pub trait CommandRunner {
    fn run(&mut self, command: &CurlCommandSpec) -> Result<CommandOutput, HttpSubmitCommandError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdCommandRunner;

impl CommandRunner for StdCommandRunner {
    fn run(&mut self, command: &CurlCommandSpec) -> Result<CommandOutput, HttpSubmitCommandError> {
        let output = Command::new(&command.program)
            .args(&command.args)
            .output()
            .map_err(|err| HttpSubmitCommandError::Io(err.to_string()))?;
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() {
            Ok(CommandOutput {
                exit_code,
                stdout,
                stderr,
            })
        } else {
            Err(HttpSubmitCommandError::NonZeroExitWithOutput {
                code: exit_code,
                stdout,
                stderr,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitExecutor {
    config: HttpSubmitClientConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSubmitter {
    request_builder: HttpSubmitRequestBuilder,
    executor: HttpSubmitExecutor,
}

impl HttpSubmitExecutor {
    pub fn new(program: impl Into<String>) -> Self {
        Self::from_config(HttpSubmitClientConfig::new(program))
    }

    pub fn from_config(config: HttpSubmitClientConfig) -> Self {
        Self { config }
    }

    pub fn from_command_config(
        command: &CommandAdapterConfig,
    ) -> Result<Self, HttpSubmitBuildError> {
        if !command.configured() {
            return Err(HttpSubmitBuildError::MissingCommandProgram);
        }

        Ok(Self::from_config(
            HttpSubmitClientConfig::from_command_config(command),
        ))
    }

    pub fn with_connect_timeout_ms(mut self, connect_timeout_ms: u64) -> Self {
        self.config.connect_timeout_ms = connect_timeout_ms.max(1);
        self
    }

    pub fn with_max_time_ms(mut self, max_time_ms: u64) -> Self {
        self.config.max_time_ms = max_time_ms.max(1);
        self
    }

    pub fn build_command(&self, spec: &HttpRequestSpec) -> CurlCommandSpec {
        let mut args = self.config.base_args.clone();
        args.extend([
            "--silent".to_string(),
            "--show-error".to_string(),
            "-X".to_string(),
            method_label(spec.method).to_string(),
            "--connect-timeout".to_string(),
            format_seconds(self.config.connect_timeout_ms),
            "--max-time".to_string(),
            format_seconds(self.config.max_time_ms),
        ]);
        if self.config.fail_on_http_error {
            args.push("--fail-with-body".to_string());
        }
        for (name, value) in &spec.headers {
            args.push("-H".to_string());
            args.push(format!("{}: {}", name, value));
        }
        args.push("--data-binary".to_string());
        args.push(spec.body.clone());
        args.push("--write-out".to_string());
        args.push(format!("{HTTP_STATUS_MARKER}%{{http_code}}"));
        args.push(spec.url.clone());
        CurlCommandSpec {
            program: self.config.program.clone(),
            args,
        }
    }

    pub fn execute<R: CommandRunner>(
        &self,
        runner: &mut R,
        spec: &HttpRequestSpec,
    ) -> Result<HttpSubmitResponse, HttpSubmitCommandError> {
        let output = match runner.run(&self.build_command(spec)) {
            Ok(output) => output,
            Err(HttpSubmitCommandError::NonZeroExitWithOutput {
                code,
                stdout,
                stderr,
            }) => {
                if let Err(HttpSubmitCommandError::HttpStatus { status_code, body }) =
                    parse_http_response(CommandOutput {
                        exit_code: code,
                        stdout: stdout.clone(),
                        stderr: stderr.clone(),
                    })
                {
                    return Err(HttpSubmitCommandError::HttpStatus { status_code, body });
                }
                return Err(HttpSubmitCommandError::NonZeroExitWithOutput {
                    code,
                    stdout,
                    stderr,
                });
            }
            Err(err) => return Err(err),
        };
        parse_http_response(output)
    }
}

impl HttpSubmitter {
    pub fn new(base_url: impl Into<String>, command_program: impl Into<String>) -> Self {
        Self::from_parts(
            HttpSubmitRequestBuilder::new(base_url),
            HttpSubmitExecutor::new(command_program),
        )
    }

    pub fn from_parts(
        request_builder: HttpSubmitRequestBuilder,
        executor: HttpSubmitExecutor,
    ) -> Self {
        Self {
            request_builder,
            executor,
        }
    }

    pub fn from_live_execution_wiring(
        wiring: &LiveExecutionWiring,
    ) -> Result<Self, HttpSubmitBuildError> {
        build_submitter(
            &wiring.submit_base_url,
            &wiring.submit,
            wiring.submit_connect_timeout_ms,
            wiring.submit_max_time_ms,
        )
    }

    pub fn from_submit_adapter_config(
        config: &SubmitAdapterConfig,
    ) -> Result<Self, HttpSubmitBuildError> {
        match config {
            SubmitAdapterConfig::Replay => Err(HttpSubmitBuildError::MissingBaseUrl),
            SubmitAdapterConfig::Http {
                base_url,
                command,
                connect_timeout_ms,
                max_time_ms,
            } => build_submitter(base_url, command, *connect_timeout_ms, *max_time_ms),
        }
    }

    pub fn from_execution_config(
        config: &ExecutionAdapterConfig,
    ) -> Result<Self, HttpSubmitBuildError> {
        Self::from_submit_adapter_config(&config.submit)
    }

    pub fn from_root(root: impl AsRef<Path>) -> Result<Self, HttpSubmitRootBuildError> {
        let execution_config =
            ExecutionAdapterConfig::from_root(root).map_err(HttpSubmitRootBuildError::Config)?;
        Self::from_execution_config(&execution_config).map_err(HttpSubmitRootBuildError::Build)
    }

    pub fn submit<R: CommandRunner>(
        &self,
        auth: &AuthRuntimeState,
        headers: &L2AuthHeaders,
        batch: &OrderBatchRequest,
        runner: &mut R,
    ) -> Result<HttpSubmitLiveResult, HttpSubmitLiveError> {
        let request = self
            .request_builder
            .build(auth, headers, batch)
            .map_err(HttpSubmitLiveError::Request)?;
        let response = self
            .executor
            .execute(runner, &request)
            .map_err(classify_live_error)?;
        Ok(HttpSubmitLiveResult { request, response })
    }

    pub fn preview_command(
        &self,
        auth: &AuthRuntimeState,
        headers: &L2AuthHeaders,
        batch: &OrderBatchRequest,
    ) -> Result<CurlCommandSpec, HttpSubmitRequestError> {
        let request = self.request_builder.build(auth, headers, batch)?;
        Ok(self.executor.build_command(&request))
    }
}

fn method_label(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Post => "POST",
    }
}

fn build_submitter(
    base_url: &str,
    command: &CommandAdapterConfig,
    connect_timeout_ms: u64,
    max_time_ms: u64,
) -> Result<HttpSubmitter, HttpSubmitBuildError> {
    if base_url.trim().is_empty() {
        return Err(HttpSubmitBuildError::MissingBaseUrl);
    }
    if !command.configured() {
        return Err(HttpSubmitBuildError::MissingCommandProgram);
    }

    Ok(HttpSubmitter::from_parts(
        HttpSubmitRequestBuilder::new(base_url),
        HttpSubmitExecutor::from_command_config(command)?
            .with_connect_timeout_ms(connect_timeout_ms)
            .with_max_time_ms(max_time_ms),
    ))
}

fn format_seconds(timeout_ms: u64) -> String {
    format!("{}.{:03}", timeout_ms / 1_000, timeout_ms % 1_000)
}

fn parse_http_response(
    output: CommandOutput,
) -> Result<HttpSubmitResponse, HttpSubmitCommandError> {
    let (body, status) = output
        .stdout
        .rsplit_once(HTTP_STATUS_MARKER)
        .ok_or(HttpSubmitCommandError::MissingStatusMarker)?;
    let status_code = status
        .trim()
        .parse::<u16>()
        .map_err(|_| HttpSubmitCommandError::InvalidStatusCode(status.trim().to_string()))?;
    let body = body.to_string();
    if (200..300).contains(&status_code) {
        Ok(HttpSubmitResponse {
            status_code,
            body,
            stderr: output.stderr,
        })
    } else {
        Err(HttpSubmitCommandError::HttpStatus { status_code, body })
    }
}

fn classify_live_error(err: HttpSubmitCommandError) -> HttpSubmitLiveError {
    match err {
        HttpSubmitCommandError::Io(message) => {
            HttpSubmitLiveError::Transport(HttpSubmitTransportError::Io(message))
        }
        HttpSubmitCommandError::NonZeroExit { code, stderr } => {
            HttpSubmitLiveError::Transport(HttpSubmitTransportError::CommandExit { code, stderr })
        }
        HttpSubmitCommandError::NonZeroExitWithOutput {
            code,
            stdout,
            stderr,
        } => HttpSubmitLiveError::Transport(HttpSubmitTransportError::CommandExitWithOutput {
            code,
            stdout,
            stderr,
        }),
        HttpSubmitCommandError::MissingStatusMarker => HttpSubmitLiveError::ResponseProtocol(
            HttpSubmitResponseProtocolError::MissingStatusMarker,
        ),
        HttpSubmitCommandError::InvalidStatusCode(status) => HttpSubmitLiveError::ResponseProtocol(
            HttpSubmitResponseProtocolError::InvalidStatusCode(status),
        ),
        HttpSubmitCommandError::HttpStatus { status_code, body } => {
            HttpSubmitLiveError::HttpStatus { status_code, body }
        }
    }
}

fn render_batch_json(batch: &OrderBatchRequest) -> String {
    let orders = batch
        .orders
        .iter()
        .map(render_envelope_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", orders)
}

fn render_envelope_json(envelope: &SignedOrderEnvelope) -> String {
    let order = &envelope.order;
    format!(
        concat!(
            "{{",
            "\"order\":{{",
            "\"maker\":\"{}\",",
            "\"signer\":\"{}\",",
            "\"taker\":\"{}\",",
            "\"tokenId\":\"{}\",",
            "\"makerAmount\":\"{}\",",
            "\"takerAmount\":\"{}\",",
            "\"side\":\"{}\",",
            "\"expiration\":\"{}\",",
            "\"nonce\":\"{}\",",
            "\"feeRateBps\":\"{}\",",
            "\"signatureType\":{},",
            "\"signature\":\"{}\",",
            "\"salt\":\"{}\"",
            "}},",
            "\"owner\":\"{}\",",
            "\"orderType\":\"{}\",",
            "\"deferExec\":{}",
            "}}"
        ),
        escape(&order.maker),
        escape(&order.signer),
        escape(&order.taker),
        escape(&order.token_id),
        escape(&order.maker_amount),
        escape(&order.taker_amount),
        escape(&order.side),
        escape(&order.expiration),
        escape(&order.nonce),
        escape(&order.fee_rate_bps),
        order.signature_type,
        escape(&order.signature),
        escape(&order.salt),
        escape(&envelope.owner),
        envelope.order_type.as_str(),
        envelope.defer_exec,
    )
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
