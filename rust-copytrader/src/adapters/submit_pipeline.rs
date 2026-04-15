use crate::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use crate::adapters::http_submit::{
    CommandOutput, CommandRunner, HttpSubmitCommandError, HttpSubmitExecutor,
    HttpSubmitRequestBuilder, HttpSubmitRequestError, OrderBatchRequest, OrderType,
};
use crate::adapters::signing::{
    AuthMaterial, AuthMaterialError, OrderSigner, SigningError, UnsignedOrderPayload,
    prepare_signed_order,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSubmitRequest {
    pub auth_material: AuthMaterial,
    pub unsigned_order: UnsignedOrderPayload,
    pub owner: String,
    pub order_type: OrderType,
    pub defer_exec: bool,
    pub sdk_available: bool,
    pub header_signature: String,
    pub header_timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitPipelineError {
    AuthMaterial(AuthMaterialError),
    Signing(SigningError),
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

    pub fn execute<S: OrderSigner, R: CommandRunner>(
        &self,
        request: PreparedSubmitRequest,
        signer: &mut S,
        runner: &mut R,
    ) -> Result<CommandOutput, SubmitPipelineError> {
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
        let headers = L2AuthHeaders::from_material(
            &request.auth_material,
            request.header_signature,
            request.header_timestamp,
        )
        .map_err(SubmitPipelineError::AuthMaterial)?;
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
