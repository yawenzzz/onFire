from __future__ import annotations


def is_tradeable_market_state(state: str) -> bool:
    return state == "OPEN"
