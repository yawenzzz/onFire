use crate::adapters::auth::L2AuthHeaders;
use crate::adapters::http_submit::{OrderType, SignedOrderEnvelope, SignedOrderPayload};
use crate::config::{RootEnvLoadError, merged_root_env};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMaterial {
    pub poly_address: String,
    pub api_key: String,
    pub api_secret: Option<String>,
    pub passphrase: String,
    pub private_key: String,
    pub signature_type: u8,
    pub funder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMaterialError {
    MissingField(String),
    FunderRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedOrderPayload {
    pub taker: String,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub side: String,
    pub expiration: String,
    pub nonce: String,
    pub fee_rate_bps: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningArtifacts {
    pub signature: String,
    pub salt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2HeaderSigningArtifacts {
    pub signature: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2HeaderSigningPayload {
    pub method: String,
    pub request_path: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub stdin: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningCommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningCommandError {
    Io(String),
    StdinWrite(String),
    NonZeroExit { code: i32, stderr: String },
    InvalidJson(String),
    MissingOutputField(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    AuthMaterial(AuthMaterialError),
    Signer(String),
    Command(SigningCommandError),
}

pub trait OrderSigner {
    fn sign_order(
        &mut self,
        payload: &UnsignedOrderPayload,
        material: &AuthMaterial,
    ) -> Result<SigningArtifacts, SigningError>;
}

pub trait L2HeaderSigner {
    fn sign_l2_headers(
        &mut self,
        payload: &L2HeaderSigningPayload,
        material: &AuthMaterial,
    ) -> Result<L2HeaderSigningArtifacts, SigningError>;
}

pub trait SigningCommandRunner {
    fn run(
        &mut self,
        command: &SigningCommandSpec,
    ) -> Result<SigningCommandOutput, SigningCommandError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdSigningCommandRunner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOrderSigner<R> {
    program: String,
    base_args: Vec<String>,
    runner: R,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandL2HeaderSigner<R> {
    program: String,
    base_args: Vec<String>,
    runner: R,
}

impl AuthMaterial {
    pub fn new(
        poly_address: impl Into<String>,
        api_key: impl Into<String>,
        passphrase: impl Into<String>,
        private_key: impl Into<String>,
        signature_type: u8,
        funder: Option<String>,
    ) -> Self {
        Self {
            poly_address: poly_address.into(),
            api_key: api_key.into(),
            api_secret: None,
            passphrase: passphrase.into(),
            private_key: private_key.into(),
            signature_type,
            funder,
        }
    }

    pub fn with_api_secret(mut self, api_secret: impl Into<String>) -> Self {
        self.api_secret = Some(api_secret.into());
        self
    }

    pub fn from_env() -> Result<Self, RootEnvLoadError> {
        Self::from_env_map(&std::env::vars().collect::<BTreeMap<_, _>>())
    }

    pub fn from_root(root: impl AsRef<Path>) -> Result<Self, RootEnvLoadError> {
        let env = merged_root_env(root)?;
        Self::from_env_map(&env)
    }

    pub fn from_env_map(env: &BTreeMap<String, String>) -> Result<Self, RootEnvLoadError> {
        let poly_address = required_env_value(env, &["POLY_ADDRESS", "SIGNER_ADDRESS"])?;
        let api_key = required_env_value(env, &["CLOB_API_KEY", "POLY_API_KEY"])?;
        let passphrase = required_env_value(env, &["CLOB_PASS_PHRASE", "POLY_PASSPHRASE"])?;
        let private_key = required_env_value(env, &["PRIVATE_KEY", "CLOB_PRIVATE_KEY"])?;
        let signature_type = optional_env_value(env, &["SIGNATURE_TYPE"])
            .map(|value| {
                value
                    .parse::<u8>()
                    .map_err(|_| RootEnvLoadError::InvalidNumber {
                        field: "SIGNATURE_TYPE".into(),
                        value,
                    })
            })
            .transpose()?
            .unwrap_or(0);
        let funder = optional_env_value(env, &["FUNDER_ADDRESS", "FUNDER"]);
        let api_secret = optional_env_value(env, &["CLOB_SECRET", "POLY_API_SECRET"]);

        let mut material = Self::new(
            poly_address,
            api_key,
            passphrase,
            private_key,
            signature_type,
            funder,
        );
        if let Some(api_secret) = api_secret {
            material = material.with_api_secret(api_secret);
        }

        Ok(material)
    }

    pub fn validate(&self) -> Result<(), AuthMaterialError> {
        if self.poly_address.is_empty() {
            Err(AuthMaterialError::MissingField("POLY_ADDRESS".into()))
        } else if self.api_key.is_empty() {
            Err(AuthMaterialError::MissingField("POLY_API_KEY".into()))
        } else if self.passphrase.is_empty() {
            Err(AuthMaterialError::MissingField("POLY_PASSPHRASE".into()))
        } else if self.private_key.is_empty() {
            Err(AuthMaterialError::MissingField("PRIVATE_KEY".into()))
        } else if self.signature_type != 0 && self.funder.as_deref().unwrap_or("").is_empty() {
            Err(AuthMaterialError::FunderRequired)
        } else {
            Ok(())
        }
    }

    pub fn validate_l2_header_ready(&self) -> Result<(), AuthMaterialError> {
        self.validate()?;
        if self.api_secret().is_none() {
            Err(AuthMaterialError::MissingField("POLY_API_SECRET".into()))
        } else {
            Ok(())
        }
    }

    fn maker_address(&self) -> &str {
        if self.signature_type == 0 {
            &self.poly_address
        } else {
            self.funder
                .as_deref()
                .expect("validated proxy signatures require funder")
        }
    }

    fn api_secret(&self) -> Option<&str> {
        self.api_secret.as_deref().filter(|value| !value.is_empty())
    }
}

impl SigningCommandRunner for StdSigningCommandRunner {
    fn run(
        &mut self,
        command: &SigningCommandSpec,
    ) -> Result<SigningCommandOutput, SigningCommandError> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .envs(command.env.iter())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| SigningCommandError::Io(err.to_string()))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(command.stdin.as_bytes())
                .map_err(|err| SigningCommandError::StdinWrite(err.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|err| SigningCommandError::Io(err.to_string()))?;
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(SigningCommandOutput {
                exit_code,
                stdout,
                stderr,
            })
        } else {
            Err(SigningCommandError::NonZeroExit {
                code: exit_code,
                stderr,
            })
        }
    }
}

impl<R> CommandOrderSigner<R> {
    pub fn new(program: impl Into<String>, base_args: Vec<String>, runner: R) -> Self {
        Self {
            program: program.into(),
            base_args,
            runner,
        }
    }

    pub fn runner(&self) -> &R {
        &self.runner
    }

    pub fn runner_mut(&mut self) -> &mut R {
        &mut self.runner
    }

    fn build_command(
        &self,
        payload: &UnsignedOrderPayload,
        material: &AuthMaterial,
    ) -> SigningCommandSpec {
        SigningCommandSpec {
            program: self.program.clone(),
            args: self.base_args.clone(),
            env: signing_env(material),
            stdin: render_signing_request_json(payload, material),
        }
    }
}

impl<R> CommandL2HeaderSigner<R> {
    pub fn new(program: impl Into<String>, base_args: Vec<String>, runner: R) -> Self {
        Self {
            program: program.into(),
            base_args,
            runner,
        }
    }

    pub fn runner(&self) -> &R {
        &self.runner
    }

    fn build_command(
        &self,
        payload: &L2HeaderSigningPayload,
        material: &AuthMaterial,
    ) -> SigningCommandSpec {
        SigningCommandSpec {
            program: self.program.clone(),
            args: self.base_args.clone(),
            env: signing_env(material),
            stdin: render_l2_header_signing_request_json(payload, material),
        }
    }
}

impl<R: SigningCommandRunner> OrderSigner for CommandOrderSigner<R> {
    fn sign_order(
        &mut self,
        payload: &UnsignedOrderPayload,
        material: &AuthMaterial,
    ) -> Result<SigningArtifacts, SigningError> {
        material.validate().map_err(SigningError::AuthMaterial)?;
        let command = self.build_command(payload, material);
        let output = self.runner.run(&command).map_err(SigningError::Command)?;
        parse_signing_output(&output.stdout).map_err(SigningError::Command)
    }
}

impl<R: SigningCommandRunner> L2HeaderSigner for CommandL2HeaderSigner<R> {
    fn sign_l2_headers(
        &mut self,
        payload: &L2HeaderSigningPayload,
        material: &AuthMaterial,
    ) -> Result<L2HeaderSigningArtifacts, SigningError> {
        material
            .validate_l2_header_ready()
            .map_err(SigningError::AuthMaterial)?;
        let command = self.build_command(payload, material);
        let output = self.runner.run(&command).map_err(SigningError::Command)?;
        parse_l2_header_signing_output(&output.stdout).map_err(SigningError::Command)
    }
}

pub fn prepare_signed_order<S: OrderSigner>(
    material: &AuthMaterial,
    unsigned: UnsignedOrderPayload,
    owner: impl Into<String>,
    order_type: OrderType,
    defer_exec: bool,
    signer: &mut S,
) -> Result<SignedOrderEnvelope, SigningError> {
    material.validate().map_err(SigningError::AuthMaterial)?;
    let signed = signer.sign_order(&unsigned, material)?;
    Ok(SignedOrderEnvelope::new(
        SignedOrderPayload {
            maker: material.maker_address().to_string(),
            signer: material.poly_address.clone(),
            taker: unsigned.taker,
            token_id: unsigned.token_id,
            maker_amount: unsigned.maker_amount,
            taker_amount: unsigned.taker_amount,
            side: unsigned.side,
            expiration: unsigned.expiration,
            nonce: unsigned.nonce,
            fee_rate_bps: unsigned.fee_rate_bps,
            signature_type: material.signature_type,
            signature: signed.signature,
            salt: signed.salt,
        },
        owner,
        order_type,
        defer_exec,
    ))
}

impl L2AuthHeaders {
    pub fn from_material(
        material: &AuthMaterial,
        poly_signature: impl Into<String>,
        poly_timestamp: impl Into<String>,
    ) -> Result<Self, AuthMaterialError> {
        material.validate()?;
        Ok(Self::new(
            material.poly_address.clone(),
            material.api_key.clone(),
            material.passphrase.clone(),
            poly_signature,
            poly_timestamp,
        ))
    }
}

pub fn prepare_l2_auth_headers<S: L2HeaderSigner>(
    material: &AuthMaterial,
    payload: L2HeaderSigningPayload,
    signer: &mut S,
) -> Result<L2AuthHeaders, SigningError> {
    material
        .validate_l2_header_ready()
        .map_err(SigningError::AuthMaterial)?;
    let signed = signer.sign_l2_headers(&payload, material)?;
    L2AuthHeaders::from_material(material, signed.signature, signed.timestamp)
        .map_err(SigningError::AuthMaterial)
}

fn signing_env(material: &AuthMaterial) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("POLY_ADDRESS".into(), material.poly_address.clone());
    env.insert("SIGNER_ADDRESS".into(), material.poly_address.clone());
    env.insert("POLY_API_KEY".into(), material.api_key.clone());
    env.insert("CLOB_API_KEY".into(), material.api_key.clone());
    if let Some(api_secret) = material.api_secret() {
        env.insert("POLY_API_SECRET".into(), api_secret.to_string());
        env.insert("CLOB_SECRET".into(), api_secret.to_string());
    }
    env.insert("POLY_PASSPHRASE".into(), material.passphrase.clone());
    env.insert("CLOB_PASS_PHRASE".into(), material.passphrase.clone());
    env.insert("PRIVATE_KEY".into(), material.private_key.clone());
    env.insert("CLOB_PRIVATE_KEY".into(), material.private_key.clone());
    env.insert("SIGNATURE_TYPE".into(), material.signature_type.to_string());

    if let Some(funder) = material.funder.as_ref().filter(|value| !value.is_empty()) {
        env.insert("FUNDER".into(), funder.clone());
        env.insert("FUNDER_ADDRESS".into(), funder.clone());
    }

    for key in [
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        env.insert(key.into(), String::new());
    }

    env
}

fn render_signing_request_json(payload: &UnsignedOrderPayload, material: &AuthMaterial) -> String {
    format!(
        concat!(
            "{{",
            "\"maker\":\"{}\",",
            "\"signer\":\"{}\",",
            "\"signatureType\":{},",
            "\"taker\":\"{}\",",
            "\"tokenId\":\"{}\",",
            "\"makerAmount\":\"{}\",",
            "\"takerAmount\":\"{}\",",
            "\"side\":\"{}\",",
            "\"expiration\":\"{}\",",
            "\"nonce\":\"{}\",",
            "\"feeRateBps\":\"{}\"",
            "}}"
        ),
        escape_json(material.maker_address()),
        escape_json(&material.poly_address),
        material.signature_type,
        escape_json(&payload.taker),
        escape_json(&payload.token_id),
        escape_json(&payload.maker_amount),
        escape_json(&payload.taker_amount),
        escape_json(&payload.side),
        escape_json(&payload.expiration),
        escape_json(&payload.nonce),
        escape_json(&payload.fee_rate_bps),
    )
}

fn render_l2_header_signing_request_json(
    payload: &L2HeaderSigningPayload,
    material: &AuthMaterial,
) -> String {
    format!(
        concat!(
            "{{",
            "\"address\":\"{}\",",
            "\"method\":\"{}\",",
            "\"requestPath\":\"{}\",",
            "\"body\":\"{}\"",
            "}}"
        ),
        escape_json(&material.poly_address),
        escape_json(&payload.method),
        escape_json(&payload.request_path),
        escape_json(&payload.body),
    )
}

fn parse_signing_output(stdout: &str) -> Result<SigningArtifacts, SigningCommandError> {
    let trimmed = stdout.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(SigningCommandError::InvalidJson(
            "expected signer stdout to be a JSON object".into(),
        ));
    }

    Ok(SigningArtifacts {
        signature: extract_json_field(trimmed, &["signature"], "signature")?,
        salt: extract_json_field(trimmed, &["salt"], "salt")?,
    })
}

fn parse_l2_header_signing_output(
    stdout: &str,
) -> Result<L2HeaderSigningArtifacts, SigningCommandError> {
    let trimmed = stdout.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(SigningCommandError::InvalidJson(
            "expected signer stdout to be a JSON object".into(),
        ));
    }

    Ok(L2HeaderSigningArtifacts {
        signature: extract_json_field(trimmed, &["signature", "POLY_SIGNATURE"], "signature")?,
        timestamp: extract_json_field(trimmed, &["timestamp", "POLY_TIMESTAMP"], "timestamp")?,
    })
}

fn extract_json_field(
    input: &str,
    fields: &[&str],
    output_field: &str,
) -> Result<String, SigningCommandError> {
    for field in fields {
        if let Some(value) = try_extract_json_field(input, field)? {
            return Ok(value);
        }
    }

    Err(SigningCommandError::MissingOutputField(output_field.into()))
}

fn try_extract_json_field(input: &str, field: &str) -> Result<Option<String>, SigningCommandError> {
    let key = format!("\"{field}\"");
    let Some(key_index) = input.find(&key) else {
        return Ok(None);
    };
    let remainder = &input[key_index + key.len()..];
    let colon_index = remainder.find(':').ok_or_else(|| {
        SigningCommandError::InvalidJson(format!("missing ':' after field {field}"))
    })?;
    let value = remainder[colon_index + 1..].trim_start();
    parse_json_stringish(value)
        .map(Some)
        .map_err(SigningCommandError::InvalidJson)
}

fn parse_json_string(input: &str) -> Result<String, String> {
    let mut chars = input.chars();
    if chars.next() != Some('"') {
        return Err("expected JSON string".into());
    }

    let mut value = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                other => return Err(format!("unsupported escape sequence: \\{other}")),
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Ok(value),
            other => value.push(other),
        }
    }

    Err("unterminated JSON string".into())
}

fn parse_json_stringish(input: &str) -> Result<String, String> {
    if input.starts_with('"') {
        return parse_json_string(input);
    }

    let value_end = input.find([',', '}']).unwrap_or(input.len());
    let value = input[..value_end].trim();

    if value.is_empty() || value == "null" || value.starts_with('{') || value.starts_with('[') {
        return Err("expected JSON string or scalar".into());
    }

    Ok(value.to_string())
}

fn optional_env_value(env: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env.get(*key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_env_value(
    env: &BTreeMap<String, String>,
    keys: &[&str],
) -> Result<String, RootEnvLoadError> {
    optional_env_value(env, keys).ok_or_else(|| RootEnvLoadError::MissingField(keys[0].to_string()))
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
