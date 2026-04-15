#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderIntent {
    pub asset_id: String,
    pub side: OrderSide,
    pub size: u64,
    pub limit_price: f64,
}

impl OrderIntent {
    pub fn new(asset_id: impl Into<String>, side: OrderSide, size: u64, limit_price: f64) -> Self {
        Self {
            asset_id: asset_id.into(),
            side,
            size,
            limit_price,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewRequest {
    pub asset_id: String,
    pub side: OrderSide,
    pub size: u64,
    pub limit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewResponse {
    pub accepted: bool,
    pub reason: Option<String>,
}

impl PreviewResponse {
    pub fn accepted() -> Self {
        Self {
            accepted: true,
            reason: None,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            accepted: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubmitRequest {
    pub asset_id: String,
    pub side: OrderSide,
    pub size: u64,
    pub limit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitResponse {
    pub accepted: bool,
    pub submitted_at_ms: Option<u64>,
    pub reason: Option<String>,
}

impl SubmitResponse {
    pub fn accepted(submitted_at_ms: u64) -> Self {
        Self {
            accepted: true,
            submitted_at_ms: Some(submitted_at_ms),
            reason: None,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            accepted: false,
            submitted_at_ms: None,
            reason: Some(reason.into()),
        }
    }
}

pub trait PreviewSubmitGateway {
    fn preview(&mut self, request: PreviewRequest) -> PreviewResponse;
    fn submit(&mut self, request: SubmitRequest) -> SubmitResponse;
}
