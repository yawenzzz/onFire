#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRuntimeState {
    creds_present: bool,
    private_key_present: bool,
    sdk_available: bool,
    signature_type: u8,
    funder_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2AuthHeaders {
    pub poly_address: String,
    pub poly_api_key: String,
    pub poly_passphrase: String,
    pub poly_signature: String,
    pub poly_timestamp: String,
}

impl AuthRuntimeState {
    pub const fn new(
        creds_present: bool,
        private_key_present: bool,
        sdk_available: bool,
        signature_type: u8,
        funder_present: bool,
    ) -> Self {
        Self {
            creds_present,
            private_key_present,
            sdk_available,
            signature_type,
            funder_present,
        }
    }

    pub fn blocked_reason(&self) -> Option<&'static str> {
        if !self.creds_present {
            Some("api_credentials_missing")
        } else if !self.private_key_present {
            Some("private_key_missing")
        } else if !self.sdk_available {
            Some("sdk_unavailable")
        } else if self.signature_type != 0 && !self.funder_present {
            Some("funder_required")
        } else {
            None
        }
    }

    pub fn submit_ready(&self) -> bool {
        self.blocked_reason().is_none()
    }

    pub fn mode_label(&self) -> &'static str {
        if self.submit_ready() {
            "account-auth-ready"
        } else {
            "account-ready"
        }
    }
}

impl L2AuthHeaders {
    pub fn new(
        poly_address: impl Into<String>,
        poly_api_key: impl Into<String>,
        poly_passphrase: impl Into<String>,
        poly_signature: impl Into<String>,
        poly_timestamp: impl Into<String>,
    ) -> Self {
        Self {
            poly_address: poly_address.into(),
            poly_api_key: poly_api_key.into(),
            poly_passphrase: poly_passphrase.into(),
            poly_signature: poly_signature.into(),
            poly_timestamp: poly_timestamp.into(),
        }
    }

    pub fn missing_header(&self) -> Option<&'static str> {
        if self.poly_address.is_empty() {
            Some("POLY_ADDRESS")
        } else if self.poly_api_key.is_empty() {
            Some("POLY_API_KEY")
        } else if self.poly_passphrase.is_empty() {
            Some("POLY_PASSPHRASE")
        } else if self.poly_signature.is_empty() {
            Some("POLY_SIGNATURE")
        } else if self.poly_timestamp.is_empty() {
            Some("POLY_TIMESTAMP")
        } else {
            None
        }
    }
}
