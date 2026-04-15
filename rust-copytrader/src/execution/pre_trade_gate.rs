use crate::adapters::order_api::{OrderIntent, OrderSide};

#[derive(Debug, Clone, PartialEq)]
pub struct PreTradeInput {
    pub asset_id: String,
    pub side: OrderSide,
    pub size: u64,
    pub limit_price: f64,
    pub market_open: bool,
    pub preview_ok: bool,
    pub remaining_budget_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreTradeGate {
    required_submit_budget_ms: u64,
}

impl PreTradeGate {
    pub const fn new(required_submit_budget_ms: u64) -> Self {
        Self {
            required_submit_budget_ms,
        }
    }

    pub fn evaluate(&self, input: PreTradeInput) -> Result<OrderIntent, String> {
        if !input.market_open {
            return Err("market_not_open".to_string());
        }

        if input.size == 0 {
            return Err("quantity_must_be_positive".to_string());
        }

        if !(0.0..=1.0).contains(&input.limit_price) || input.limit_price == 0.0 {
            return Err("price_out_of_range".to_string());
        }

        if input.remaining_budget_ms < self.required_submit_budget_ms {
            return Err("latency_budget_exhausted".to_string());
        }

        Ok(OrderIntent::new(
            input.asset_id,
            input.side,
            input.size,
            input.limit_price,
        ))
    }
}
