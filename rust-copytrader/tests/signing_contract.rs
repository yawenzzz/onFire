use rust_copytrader::adapters::auth::L2AuthHeaders;
use rust_copytrader::adapters::http_submit::OrderType;
use rust_copytrader::adapters::signing::{
    AuthMaterial, AuthMaterialError, OrderSigner, SigningArtifacts, SigningError,
    UnsignedOrderPayload, prepare_signed_order,
};

#[test]
fn auth_material_validation_requires_funder_for_non_default_signature_type() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        2,
        None,
    );

    let err = material.validate().unwrap_err();

    assert_eq!(err, AuthMaterialError::FunderRequired);
}

#[test]
fn prepare_signed_order_uses_auth_identity_and_signer_output() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let unsigned = UnsignedOrderPayload {
        taker: "0x0000000000000000000000000000000000000000".into(),
        token_id: "12345".into(),
        maker_amount: "1000000".into(),
        taker_amount: "2000000".into(),
        side: "BUY".into(),
        expiration: "1735689600".into(),
        nonce: "7".into(),
        fee_rate_bps: "30".into(),
    };
    let mut signer = StubSigner::success("0xsig", "999");

    let envelope = prepare_signed_order(
        &material,
        unsigned,
        "owner-uuid",
        OrderType::Gtc,
        false,
        &mut signer,
    )
    .expect("signed order envelope");

    assert_eq!(envelope.owner, "owner-uuid");
    assert_eq!(envelope.order_type, OrderType::Gtc);
    assert!(!envelope.defer_exec);
    assert_eq!(envelope.order.maker, "0xpoly-address");
    assert_eq!(envelope.order.signer, "0xpoly-address");
    assert_eq!(envelope.order.signature_type, 0);
    assert_eq!(envelope.order.signature, "0xsig");
    assert_eq!(envelope.order.salt, "999");
    assert_eq!(signer.calls, 1);
}

#[test]
fn prepare_signed_order_surfaces_signer_failure() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let unsigned = UnsignedOrderPayload {
        taker: "0x0000000000000000000000000000000000000000".into(),
        token_id: "12345".into(),
        maker_amount: "1000000".into(),
        taker_amount: "2000000".into(),
        side: "BUY".into(),
        expiration: "1735689600".into(),
        nonce: "7".into(),
        fee_rate_bps: "30".into(),
    };
    let mut signer = StubSigner::failure("signature_failed");

    let err = prepare_signed_order(
        &material,
        unsigned,
        "owner-uuid",
        OrderType::Gtc,
        false,
        &mut signer,
    )
    .unwrap_err();

    assert_eq!(err, SigningError::Signer("signature_failed".into()));
}

#[test]
fn l2_auth_headers_can_be_derived_from_auth_material() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );

    let headers = L2AuthHeaders::from_material(&material, "0xheader-sig", "1712345678")
        .expect("l2 auth headers");

    assert_eq!(headers.poly_address, "0xpoly-address");
    assert_eq!(headers.poly_api_key, "api-key");
    assert_eq!(headers.poly_passphrase, "passphrase");
    assert_eq!(headers.poly_signature, "0xheader-sig");
    assert_eq!(headers.poly_timestamp, "1712345678");
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
