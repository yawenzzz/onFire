use rust_copytrader::adapters::auth::L2AuthHeaders;
use rust_copytrader::adapters::http_submit::OrderType;
use rust_copytrader::adapters::signing::{
    AuthMaterial, AuthMaterialError, CommandL2HeaderSigner, CommandOrderSigner,
    L2HeaderSigningPayload, OrderSigner, SigningArtifacts, SigningCommandError,
    SigningCommandOutput, SigningCommandRunner, SigningCommandSpec, SigningError,
    UnsignedOrderPayload, prepare_l2_auth_headers, prepare_signed_order,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn prepare_signed_order_uses_funder_as_maker_for_proxy_signatures() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        2,
        Some("0xfunder-address".into()),
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

    assert_eq!(envelope.order.maker, "0xfunder-address");
    assert_eq!(envelope.order.signer, "0xpoly-address");
    assert_eq!(envelope.order.signature_type, 2);
    assert_eq!(signer.calls, 1);
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

#[test]
fn l2_auth_header_signing_requires_api_secret() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    );
    let mut signer = CommandL2HeaderSigner::new(
        "python3",
        vec!["scripts/sign_l2.py".into()],
        StubCommandRunner::success("{}"),
    );

    let err = prepare_l2_auth_headers(
        &material,
        L2HeaderSigningPayload {
            method: "POST".into(),
            request_path: "/orders".into(),
            body: "{\"market\":\"yes\"}".into(),
        },
        &mut signer,
    )
    .unwrap_err();

    assert_eq!(
        err,
        SigningError::AuthMaterial(AuthMaterialError::MissingField("POLY_API_SECRET".into()))
    );
}

#[test]
fn command_l2_header_signer_builds_secret_aware_env_bridge_and_headers() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    )
    .with_api_secret("api-secret");
    let runner =
        StubCommandRunner::success("{\"signature\":\"0xl2-sig\",\"timestamp\":\"1712345678\"}");
    let mut signer = CommandL2HeaderSigner::new(
        "python3",
        vec!["scripts/sign_l2.py".into(), "--json".into()],
        runner,
    );

    let headers = prepare_l2_auth_headers(
        &material,
        L2HeaderSigningPayload {
            method: "POST".into(),
            request_path: "/orders".into(),
            body: "{\"owner\":\"owner-uuid\"}".into(),
        },
        &mut signer,
    )
    .expect("command-backed L2 header signing succeeds");

    assert_eq!(headers.poly_address, "0xpoly-address");
    assert_eq!(headers.poly_api_key, "api-key");
    assert_eq!(headers.poly_passphrase, "passphrase");
    assert_eq!(headers.poly_signature, "0xl2-sig");
    assert_eq!(headers.poly_timestamp, "1712345678");

    let command = signer
        .runner()
        .last_command
        .as_ref()
        .expect("l2 command captured");
    assert_eq!(command.program, "python3");
    assert_eq!(command.args, vec!["scripts/sign_l2.py", "--json"]);
    assert_eq!(
        command.env.get("CLOB_SECRET").map(String::as_str),
        Some("api-secret")
    );
    assert_eq!(
        command.env.get("POLY_API_SECRET").map(String::as_str),
        Some("api-secret")
    );
    assert_eq!(
        command.env.get("CLOB_API_KEY").map(String::as_str),
        Some("api-key")
    );
    assert_eq!(
        command.env.get("CLOB_PASS_PHRASE").map(String::as_str),
        Some("passphrase")
    );
    assert_eq!(command.env.get("ALL_PROXY").map(String::as_str), Some(""));
    assert_eq!(command.env.get("https_proxy").map(String::as_str), Some(""));
    assert!(command.stdin.contains("\"method\":\"POST\""));
    assert!(command.stdin.contains("\"requestPath\":\"/orders\""));
    assert!(command.stdin.contains("\\\"owner\\\":\\\"owner-uuid\\\""));
}

#[test]
fn command_l2_header_signer_accepts_py_clob_client_header_map_output() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        0,
        None,
    )
    .with_api_secret("api-secret");
    let runner = StubCommandRunner::success(
        "{\"POLY_ADDRESS\":\"0xpoly-address\",\"POLY_API_KEY\":\"api-key\",\"POLY_PASSPHRASE\":\"passphrase\",\"POLY_SIGNATURE\":\"0xl2-sig\",\"POLY_TIMESTAMP\":1712345678}",
    );
    let mut signer = CommandL2HeaderSigner::new(
        "python3",
        vec!["scripts/sign_l2.py".into(), "--json".into()],
        runner,
    );

    let headers = prepare_l2_auth_headers(
        &material,
        L2HeaderSigningPayload {
            method: "POST".into(),
            request_path: "/orders".into(),
            body: "{\"owner\":\"owner-uuid\"}".into(),
        },
        &mut signer,
    )
    .expect("py-clob-client style header map output should be accepted");

    assert_eq!(headers.poly_signature, "0xl2-sig");
    assert_eq!(headers.poly_timestamp, "1712345678");
}

#[test]
fn command_order_signer_builds_realistic_env_bridge_and_parses_output() {
    let material = AuthMaterial::new(
        "0xpoly-address",
        "api-key",
        "passphrase",
        "private-key",
        2,
        Some("0xfunder-address".into()),
    )
    .with_api_secret("api-secret");
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
    let runner = StubCommandRunner::success("{\"signature\":\"0xcmd-sig\",\"salt\":\"555\"}");
    let mut signer = CommandOrderSigner::new(
        "python3",
        vec!["scripts/sign_order.py".into(), "--json".into()],
        runner,
    );

    let artifacts = signer
        .sign_order(&unsigned, &material)
        .expect("command-backed signing succeeds");

    assert_eq!(
        artifacts,
        SigningArtifacts {
            signature: "0xcmd-sig".into(),
            salt: "555".into(),
        }
    );

    let command = signer
        .runner()
        .last_command
        .as_ref()
        .expect("signing command captured");
    assert_eq!(command.program, "python3");
    assert_eq!(command.args, vec!["scripts/sign_order.py", "--json"]);
    assert_eq!(
        command.env.get("CLOB_PRIVATE_KEY").map(String::as_str),
        Some("private-key")
    );
    assert_eq!(
        command.env.get("CLOB_SECRET").map(String::as_str),
        Some("api-secret")
    );
    assert_eq!(
        command.env.get("FUNDER_ADDRESS").map(String::as_str),
        Some("0xfunder-address")
    );
    assert_eq!(
        command.env.get("SIGNATURE_TYPE").map(String::as_str),
        Some("2")
    );
    assert_eq!(command.env.get("ALL_PROXY").map(String::as_str), Some(""));
    assert_eq!(command.env.get("http_proxy").map(String::as_str), Some(""));
    assert!(command.stdin.contains("\"tokenId\":\"12345\""));
    assert!(command.stdin.contains("\"maker\":\"0xfunder-address\""));
    assert!(command.stdin.contains("\"signer\":\"0xpoly-address\""));
}

#[test]
fn command_order_signer_accepts_numeric_salt_from_helper_output() {
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
    let runner =
        StubCommandRunner::success("{\"order\":{\"signature\":\"0xcmd-sig\",\"salt\":555}}");
    let mut signer =
        CommandOrderSigner::new("python3", vec!["scripts/sign_order.py".into()], runner);

    let artifacts = signer
        .sign_order(&unsigned, &material)
        .expect("numeric helper salt should be accepted");

    assert_eq!(
        artifacts,
        SigningArtifacts {
            signature: "0xcmd-sig".into(),
            salt: "555".into(),
        }
    );
}

#[test]
fn auth_material_loads_from_root_env_files_with_brownfield_aliases() {
    let root = unique_temp_root("signing-auth-material");
    fs::create_dir_all(&root).expect("temp root created");
    fs::write(
        root.join(".env"),
        concat!(
            "POLY_ADDRESS=0xpoly-address\n",
            "CLOB_API_KEY=env-key\n",
            "CLOB_PASS_PHRASE=env-passphrase\n",
            "PRIVATE_KEY=env-private-key\n",
            "SIGNATURE_TYPE=0\n",
        ),
    )
    .expect(".env written");
    fs::write(
        root.join(".env.local"),
        concat!(
            "CLOB_API_KEY=local-key\n",
            "CLOB_SECRET=local-secret\n",
            "CLOB_PRIVATE_KEY=local-private-key\n",
            "SIGNATURE_TYPE=2\n",
            "FUNDER_ADDRESS=0xfunder-address\n",
        ),
    )
    .expect(".env.local written");

    let material = AuthMaterial::from_root(&root).expect("auth material from root");

    assert_eq!(material.poly_address, "0xpoly-address");
    assert_eq!(material.api_key, "local-key");
    assert_eq!(material.api_secret.as_deref(), Some("local-secret"));
    assert_eq!(material.passphrase, "env-passphrase");
    assert_eq!(material.private_key, "env-private-key");
    assert_eq!(material.signature_type, 2);
    assert_eq!(material.funder.as_deref(), Some("0xfunder-address"));

    fs::remove_dir_all(root).expect("temp root removed");
}

#[test]
fn command_order_signer_fails_closed_on_invalid_output() {
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
    let runner = StubCommandRunner::success("{\"signature\":\"0xcmd-sig\"}");
    let mut signer =
        CommandOrderSigner::new("python3", vec!["scripts/sign_order.py".into()], runner);

    let err = signer.sign_order(&unsigned, &material).unwrap_err();

    assert_eq!(
        err,
        SigningError::Command(SigningCommandError::MissingOutputField("salt".into()))
    );
}

struct StubSigner {
    result: Result<SigningArtifacts, SigningError>,
    calls: usize,
}

fn unique_temp_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rust-copytrader-{name}-{suffix}"))
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StubCommandRunner {
    result: Result<SigningCommandOutput, SigningCommandError>,
    last_command: Option<SigningCommandSpec>,
}

impl StubCommandRunner {
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

impl SigningCommandRunner for StubCommandRunner {
    fn run(
        &mut self,
        command: &SigningCommandSpec,
    ) -> Result<SigningCommandOutput, SigningCommandError> {
        self.last_command = Some(command.clone());
        self.result.clone()
    }
}
