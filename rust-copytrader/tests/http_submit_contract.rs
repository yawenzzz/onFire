use rust_copytrader::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use rust_copytrader::adapters::http_submit::{
    HttpMethod, HttpSubmitLiveError, HttpSubmitRequestBuilder, HttpSubmitRequestError,
    HttpSubmitter, OrderBatchRequest, OrderType, SignedOrderEnvelope, SignedOrderPayload,
};
use rust_copytrader::config::ExecutionAdapterConfig;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn sample_payload() -> SignedOrderPayload {
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
    }
}

fn unique_temp_root(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nanos))
}

#[test]
fn builds_authenticated_orders_request_for_single_signed_order() {
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let builder = HttpSubmitRequestBuilder::new("https://clob.polymarket.com");
    let request = OrderBatchRequest::single(SignedOrderEnvelope::new(
        sample_payload(),
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));

    let spec = builder
        .build(&auth, &headers, &request)
        .expect("request spec");

    assert_eq!(spec.method, HttpMethod::Post);
    assert_eq!(spec.url, "https://clob.polymarket.com/orders");
    assert_eq!(
        spec.headers.get("Content-Type").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(
        spec.headers.get("Accept").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(
        spec.headers.get("POLY_ADDRESS").map(String::as_str),
        Some("0xpoly-address")
    );
    assert_eq!(
        spec.headers.get("POLY_API_KEY").map(String::as_str),
        Some("api-key")
    );
    assert_eq!(
        spec.headers.get("POLY_PASSPHRASE").map(String::as_str),
        Some("passphrase")
    );
    assert_eq!(
        spec.headers.get("POLY_SIGNATURE").map(String::as_str),
        Some("signature")
    );
    assert_eq!(
        spec.headers.get("POLY_TIMESTAMP").map(String::as_str),
        Some("1712345678")
    );
    assert!(spec.body.contains("\"owner\":\"owner-uuid\""));
    assert!(spec.body.contains("\"orderType\":\"GTC\""));
    assert!(spec.body.contains("\"tokenId\":\"12345\""));
}

#[test]
fn rejects_http_submit_when_auth_runtime_is_not_ready() {
    let auth = AuthRuntimeState::new(true, false, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let builder = HttpSubmitRequestBuilder::new("https://clob.polymarket.com");
    let request = OrderBatchRequest::single(SignedOrderEnvelope::new(
        sample_payload(),
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));

    let err = builder.build(&auth, &headers, &request).unwrap_err();
    assert_eq!(
        err,
        HttpSubmitRequestError::ExecutionSurfaceNotReady("private_key_missing".into())
    );
}

#[test]
fn rejects_http_submit_when_required_l2_headers_are_missing() {
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "",
        "passphrase",
        "signature",
        "1712345678",
    );
    let builder = HttpSubmitRequestBuilder::new("https://clob.polymarket.com");
    let request = OrderBatchRequest::single(SignedOrderEnvelope::new(
        sample_payload(),
        "owner-uuid",
        OrderType::Fak,
        true,
    ));

    let err = builder.build(&auth, &headers, &request).unwrap_err();
    assert_eq!(
        err,
        HttpSubmitRequestError::MissingHeader("POLY_API_KEY".into())
    );
}

#[test]
fn request_builder_uses_loaded_submit_config_without_hand_built_live_wiring() {
    let root = unique_temp_root("http-submit-builder-root");
    fs::create_dir_all(&root).expect("temp root created");
    fs::write(
        root.join(".env.local"),
        concat!(
            "RUST_COPYTRADER_SIGNING_PROGRAM=python3\n",
            "RUST_COPYTRADER_SUBMIT_PROGRAM=curl\n",
            "CLOB_BASE_URL=https://helper.polymarket.test/\n",
        ),
    )
    .expect(".env.local written");
    let execution_config = ExecutionAdapterConfig::from_root(&root).expect("execution config");
    let builder = HttpSubmitRequestBuilder::from_submit_adapter_config(&execution_config.submit)
        .expect("builder from submit config");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let request = OrderBatchRequest::single(SignedOrderEnvelope::new(
        sample_payload(),
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));

    let spec = builder
        .build(&auth, &headers, &request)
        .expect("request spec");

    assert_eq!(spec.url, "https://helper.polymarket.test/orders");

    fs::remove_dir_all(root).expect("temp root removed");
}

#[test]
fn live_submitter_rejects_request_surface_errors_before_running_command() {
    let auth = AuthRuntimeState::new(true, false, true, 0, false);
    let headers = L2AuthHeaders::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "signature",
        "1712345678",
    );
    let submitter = HttpSubmitter::new("https://clob.polymarket.com", "curl");
    let request = OrderBatchRequest::single(SignedOrderEnvelope::new(
        sample_payload(),
        "owner-uuid",
        OrderType::Gtc,
        false,
    ));

    let err = submitter
        .submit(&auth, &headers, &request, &mut PanicRunner)
        .unwrap_err();

    assert_eq!(
        err,
        HttpSubmitLiveError::Request(HttpSubmitRequestError::ExecutionSurfaceNotReady(
            "private_key_missing".into()
        ))
    );
}

#[derive(Debug, Default)]
struct PanicRunner;

impl rust_copytrader::adapters::http_submit::CommandRunner for PanicRunner {
    fn run(
        &mut self,
        _command: &rust_copytrader::adapters::http_submit::CurlCommandSpec,
    ) -> Result<
        rust_copytrader::adapters::http_submit::CommandOutput,
        rust_copytrader::adapters::http_submit::HttpSubmitCommandError,
    > {
        panic!("request errors should short-circuit before command execution");
    }
}
