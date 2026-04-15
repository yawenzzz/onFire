use crate::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use std::collections::BTreeMap;

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
