use crate::adapters::auth::L2AuthHeaders;
use crate::adapters::http_submit::{OrderType, SignedOrderEnvelope, SignedOrderPayload};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMaterial {
    pub poly_address: String,
    pub api_key: String,
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
pub enum SigningError {
    AuthMaterial(AuthMaterialError),
    Signer(String),
}

pub trait OrderSigner {
    fn sign_order(
        &mut self,
        payload: &UnsignedOrderPayload,
        material: &AuthMaterial,
    ) -> Result<SigningArtifacts, SigningError>;
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
            passphrase: passphrase.into(),
            private_key: private_key.into(),
            signature_type,
            funder,
        }
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
            maker: material.poly_address.clone(),
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
