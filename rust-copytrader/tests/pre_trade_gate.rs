use rust_copytrader::adapters::order_api::OrderSide;
use rust_copytrader::execution::pre_trade_gate::{PreTradeGate, PreTradeInput};

#[test]
fn pre_trade_gate_rejects_when_remaining_budget_cannot_cover_submit_stage() {
    let gate = PreTradeGate::new(70);
    let rejected = gate.evaluate(PreTradeInput {
        asset_id: "asset-9".into(),
        side: OrderSide::Buy,
        size: 4,
        limit_price: 0.52,
        market_open: true,
        preview_ok: true,
        remaining_budget_ms: 35,
    });

    assert_eq!(rejected.unwrap_err(), "latency_budget_exhausted");
}

#[test]
fn pre_trade_gate_rejects_market_closed_and_invalid_price_shape() {
    let gate = PreTradeGate::new(70);
    let market_closed = gate.evaluate(PreTradeInput {
        asset_id: "asset-9".into(),
        side: OrderSide::Sell,
        size: 4,
        limit_price: 0.48,
        market_open: false,
        preview_ok: true,
        remaining_budget_ms: 90,
    });
    assert_eq!(market_closed.unwrap_err(), "market_not_open");

    let invalid_price = gate.evaluate(PreTradeInput {
        asset_id: "asset-9".into(),
        side: OrderSide::Sell,
        size: 4,
        limit_price: 1.25,
        market_open: true,
        preview_ok: true,
        remaining_budget_ms: 90,
    });
    assert_eq!(invalid_price.unwrap_err(), "price_out_of_range");

    let zero_size = gate.evaluate(PreTradeInput {
        asset_id: "asset-9".into(),
        side: OrderSide::Sell,
        size: 0,
        limit_price: 0.48,
        market_open: true,
        preview_ok: true,
        remaining_budget_ms: 90,
    });
    assert_eq!(zero_size.unwrap_err(), "quantity_must_be_positive");
}

#[test]
fn pre_trade_gate_builds_submit_ready_intent_for_valid_inputs() {
    let gate = PreTradeGate::new(70);
    let intent = gate
        .evaluate(PreTradeInput {
            asset_id: "asset-9".into(),
            side: OrderSide::Buy,
            size: 4,
            limit_price: 0.52,
            market_open: true,
            preview_ok: true,
            remaining_budget_ms: 90,
        })
        .unwrap();

    assert_eq!(intent.asset_id, "asset-9");
    assert_eq!(intent.side, OrderSide::Buy);
    assert_eq!(intent.size, 4);
    assert_eq!(intent.limit_price, 0.52);
}
