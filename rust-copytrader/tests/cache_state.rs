use rust_copytrader::adapters::market_ws::MarketQuoteSnapshot;
use rust_copytrader::adapters::positions::PositionSnapshot;
use rust_copytrader::cache::leader_state::{LeaderMarketState, LeaderStateCache, LeaderStateError};
use rust_copytrader::cache::market_cache::{MarketCache, MarketCacheError};
use rust_copytrader::domain::events::ActivityEvent;

#[test]
fn market_cache_keeps_the_newest_quote_per_asset() {
    let mut cache = MarketCache::new(10);
    cache.update(MarketQuoteSnapshot {
        asset_id: "asset-9".into(),
        best_bid: 0.48,
        best_ask: 0.52,
        observed_at_ms: 1_000,
    });
    cache.update(MarketQuoteSnapshot {
        asset_id: "asset-9".into(),
        best_bid: 0.49,
        best_ask: 0.53,
        observed_at_ms: 1_005,
    });
    cache.update(MarketQuoteSnapshot {
        asset_id: "asset-9".into(),
        best_bid: 0.47,
        best_ask: 0.51,
        observed_at_ms: 999,
    });

    let quote = cache.quote("asset-9", 1_010).unwrap();

    assert_eq!(quote.asset_id, "asset-9");
    assert_eq!(quote.best_bid, 0.49);
    assert_eq!(quote.best_ask, 0.53);
    assert_eq!(quote.observed_at_ms, 1_005);
}

#[test]
fn market_cache_rejects_stale_quote_reads() {
    let mut cache = MarketCache::new(10);
    cache.update(MarketQuoteSnapshot {
        asset_id: "asset-9".into(),
        best_bid: 0.48,
        best_ask: 0.52,
        observed_at_ms: 1_000,
    });

    let error = cache.quote("asset-9", 1_011).unwrap_err();

    assert_eq!(
        error,
        MarketCacheError::Stale {
            asset_id: "asset-9".into(),
            observed_age_ms: 11,
        }
    );
}

#[test]
fn leader_state_updates_activity_and_position_for_a_market() {
    let mut cache = LeaderStateCache::new(25);
    cache.record_activity(ActivityEvent::new(
        "leader-1",
        "0xtx-1",
        "BUY",
        "asset-9",
        4,
        1_000,
    ));
    cache.update_position(PositionSnapshot::new("leader-1", "asset-9", 12, 1_004, 2));
    cache.record_activity(ActivityEvent::new(
        "leader-1",
        "0xtx-2",
        "BUY",
        "asset-9",
        6,
        1_010,
    ));
    cache.update_position(PositionSnapshot::new("leader-1", "asset-9", 18, 1_012, 1));
    cache.record_activity(ActivityEvent::new(
        "leader-1",
        "0xtx-older",
        "BUY",
        "asset-9",
        1,
        1_001,
    ));

    let state = cache.market_state("leader-1", "asset-9", 1_020).unwrap();

    assert_eq!(
        state,
        LeaderMarketState {
            leader_id: "leader-1".into(),
            asset_id: "asset-9".into(),
            last_activity_at_ms: 1_010,
            last_transaction_hash: "0xtx-2".into(),
            last_position_size: 18,
            position_observed_at_ms: 1_012,
        }
    );
}

#[test]
fn leader_state_rejects_stale_position_reads() {
    let mut cache = LeaderStateCache::new(10);
    cache.record_activity(ActivityEvent::new(
        "leader-1",
        "0xtx-1",
        "SELL",
        "asset-9",
        2,
        1_000,
    ));
    cache.update_position(PositionSnapshot::new("leader-1", "asset-9", 12, 1_004, 8));

    let error = cache.market_state("leader-1", "asset-9", 1_010).unwrap_err();

    assert_eq!(
        error,
        LeaderStateError::StalePosition {
            leader_id: "leader-1".into(),
            asset_id: "asset-9".into(),
            observed_age_ms: 14,
        }
    );
}
