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
