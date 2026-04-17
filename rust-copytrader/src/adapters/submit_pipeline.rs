use crate::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use crate::adapters::http_submit::{
    CommandRunner, HttpSubmitBuildError, HttpSubmitClientConfig, HttpSubmitCommandError,
    HttpSubmitExecutor, HttpSubmitRequestBuilder, HttpSubmitRequestError, HttpSubmitResponse,
    OrderBatchRequest, OrderType,
};
use crate::adapters::signing::{
    AuthMaterial, AuthMaterialError, OrderSigner, SigningError, UnsignedOrderPayload,
    prepare_signed_order,
};
use crate::config::LiveExecutionWiring;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSubmitRequest {
    pub auth_material: AuthMaterial,
    pub unsigned_order: UnsignedOrderPayload,
    pub owner: String,
    pub order_type: OrderType,
    pub defer_exec: bool,
    pub sdk_available: bool,
}

pub trait L2HeaderProvider {
    fn l2_headers(&mut self, material: &AuthMaterial) -> Result<L2AuthHeaders, SigningError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitPipelineError {
    AuthMaterial(AuthMaterialError),
    Signing(SigningError),
    HeaderSigning(SigningError),
    Request(HttpSubmitRequestError),
    Command(HttpSubmitCommandError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitPipeline {
    request_builder: HttpSubmitRequestBuilder,
    submit_executor: HttpSubmitExecutor,
}

impl SubmitPipeline {
    pub fn new(base_url: impl Into<String>, command_program: impl Into<String>) -> Self {
        Self {
            request_builder: HttpSubmitRequestBuilder::new(base_url),
            submit_executor: HttpSubmitExecutor::new(command_program),
        }
    }

    pub fn from_live_execution_wiring(
        wiring: &LiveExecutionWiring,
    ) -> Result<Self, HttpSubmitBuildError> {
        if wiring.submit_base_url.trim().is_empty() {
            return Err(HttpSubmitBuildError::MissingBaseUrl);
        }
        if !wiring.submit.configured() {
            return Err(HttpSubmitBuildError::MissingCommandProgram);
        }

        Ok(Self {
            request_builder: HttpSubmitRequestBuilder::new(&wiring.submit_base_url),
            submit_executor: HttpSubmitExecutor::from_config(
                HttpSubmitClientConfig::new(wiring.submit.program.clone())
                    .with_connect_timeout_ms(wiring.submit_connect_timeout_ms)
                    .with_max_time_ms(wiring.submit_max_time_ms),
            ),
        })
    }

    pub fn execute<S: OrderSigner, H: L2HeaderProvider, R: CommandRunner>(
        &self,
        request: PreparedSubmitRequest,
        signer: &mut S,
        header_provider: &mut H,
        runner: &mut R,
    ) -> Result<HttpSubmitResponse, SubmitPipelineError> {
        request
            .auth_material
            .validate()
            .map_err(SubmitPipelineError::AuthMaterial)?;

        let signed = prepare_signed_order(
            &request.auth_material,
            request.unsigned_order,
            request.owner,
            request.order_type,
            request.defer_exec,
            signer,
        )
        .map_err(SubmitPipelineError::Signing)?;

        let auth = AuthRuntimeState::new(
            !request.auth_material.api_key.is_empty()
                && !request.auth_material.passphrase.is_empty(),
            !request.auth_material.private_key.is_empty(),
            request.sdk_available,
            request.auth_material.signature_type,
            request.auth_material.funder.is_some(),
        );
        let headers = header_provider
            .l2_headers(&request.auth_material)
            .map_err(SubmitPipelineError::HeaderSigning)?;
        let batch = OrderBatchRequest::single(signed);
        let spec = self
            .request_builder
            .build(&auth, &headers, &batch)
            .map_err(SubmitPipelineError::Request)?;
        self.submit_executor
            .execute(runner, &spec)
            .map_err(SubmitPipelineError::Command)
    }
}
