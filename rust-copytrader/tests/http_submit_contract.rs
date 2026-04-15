use rust_copytrader::adapters::auth::{AuthRuntimeState, L2AuthHeaders};
use rust_copytrader::adapters::http_submit::{
    HttpMethod, HttpSubmitRequestBuilder, HttpSubmitRequestError, OrderBatchRequest, OrderType,
    SignedOrderEnvelope, SignedOrderPayload,
};

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
