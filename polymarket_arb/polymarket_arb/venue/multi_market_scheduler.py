from __future__ import annotations

from polymarket_arb.venue.subscription_planner import chunk_market_ids
from polymarket_arb.venue.ws_subscription import build_market_subscription_message


def build_subscription_batches(market_ids: list[str], chunk_size: int) -> list[dict]:
    return [build_market_subscription_message(chunk) for chunk in chunk_market_ids(market_ids, chunk_size)]
