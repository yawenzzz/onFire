#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRuntimeState {
    creds_present: bool,
    private_key_present: bool,
    sdk_available: bool,
    signature_type: u8,
    funder_present: bool,
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
