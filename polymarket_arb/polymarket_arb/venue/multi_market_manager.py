from __future__ import annotations

from polymarket_arb.venue.multi_market_scheduler import build_subscription_batches


def build_subscription_plan(market_ids: list[str], chunk_size: int) -> dict:
    subscriptions = build_subscription_batches(market_ids, chunk_size)
    return {
        'market_count': len(market_ids),
        'subscriptions': subscriptions,
    }
