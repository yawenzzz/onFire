from __future__ import annotations

from polymarket_arb.data.contracts import MarketSnapshot

REQUIRED_FIELDS = {'market_id', 'market_state', 'best_bid', 'best_ask'}


def normalize_market_message(message: dict) -> MarketSnapshot:
    missing = sorted(REQUIRED_FIELDS - set(message.keys()))
    if missing:
        raise ValueError(f'missing required fields: {", ".join(missing)}')
    return MarketSnapshot(
        market_id=message['market_id'],
        market_state=message['market_state'],
        best_bid=message['best_bid'],
        best_ask=message['best_ask'],
    )
