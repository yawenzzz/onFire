use rust_copytrader::adapters::market_ws::{MarketQuoteGate, QuoteRejection};
use rust_copytrader::adapters::positions::{
    PositionSnapshot, PositionsOutcome, PositionsReconciler,
};

#[test]
fn positions_reconciler_rejects_stale_snapshot() {
    let reconciler = PositionsReconciler::new(45);
    let previous = PositionSnapshot::new("leader-1", "asset-9", 10, 1_000, 1_000);
    let current = PositionSnapshot::new("leader-1", "asset-9", 12, 1_060, 60);

    let outcome = reconciler.reconcile(&previous, &current);

    assert_eq!(
        outcome,
        PositionsOutcome::Rejected("positions_snapshot_stale".into())
    );
}

#[test]
fn positions_reconciler_emits_delta_when_size_changes() {
    let reconciler = PositionsReconciler::new(45);
    let previous = PositionSnapshot::new("leader-1", "asset-9", 10, 1_000, 5);
    let current = PositionSnapshot::new("leader-1", "asset-9", 14, 1_025, 8);

    let outcome = reconciler.reconcile(&previous, &current);

    match outcome {
        PositionsOutcome::Delta(delta) => {
            assert_eq!(delta.proxy_wallet, "leader-1");
            assert_eq!(delta.asset_id, "asset-9");
            assert_eq!(delta.previous_size, 10);
            assert_eq!(delta.current_size, 14);
            assert_eq!(delta.delta_size, 4);
        }
        other => panic!("expected delta, got {other:?}"),
    }
}

#[test]
fn positions_reconciler_rejects_when_no_net_change_exists() {
    let reconciler = PositionsReconciler::new(45);
    let previous = PositionSnapshot::new("leader-1", "asset-9", 10, 1_000, 2);
    let current = PositionSnapshot::new("leader-1", "asset-9", 10, 1_025, 3);

    let outcome = reconciler.reconcile(&previous, &current);

    assert_eq!(outcome, PositionsOutcome::NoNetChange);
}

#[test]
fn positions_reconciler_rejects_subject_mismatch() {
    let reconciler = PositionsReconciler::new(45);
    let previous = PositionSnapshot::new("leader-1", "asset-9", 10, 1_000, 2);
    let current = PositionSnapshot::new("leader-2", "asset-9", 12, 1_025, 3);

    let outcome = reconciler.reconcile(&previous, &current);

    assert_eq!(
        outcome,
        PositionsOutcome::Rejected("positions_subject_mismatch".into())
    );
}

#[test]
fn market_quote_gate_rejects_stale_quotes() {
    let gate = MarketQuoteGate::new(10);
    let rejected = gate.validate("asset-9", 0.48, 0.52, 15, 1_050);

    assert_eq!(rejected, Err(QuoteRejection::Stale));
}

#[test]
fn market_quote_gate_rejects_invalid_spread() {
    let gate = MarketQuoteGate::new(10);
    let rejected = gate.validate("asset-9", 0.53, 0.52, 5, 1_050);

    assert_eq!(rejected, Err(QuoteRejection::InvalidSpread));
}

#[test]
fn market_quote_gate_accepts_fresh_quotes() {
    let gate = MarketQuoteGate::new(10);
    let quote = gate.validate("asset-9", 0.48, 0.52, 9, 1_050).unwrap();

    assert_eq!(quote.asset_id, "asset-9");
    assert_eq!(quote.best_bid, 0.48);
    assert_eq!(quote.best_ask, 0.52);
    assert_eq!(quote.observed_at_ms, 1_050);
}
