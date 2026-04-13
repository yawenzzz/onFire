from __future__ import annotations


def chunk_market_ids(market_ids: list[str], chunk_size: int) -> list[list[str]]:
    if chunk_size <= 0:
        raise ValueError('chunk_size must be positive')
    return [market_ids[i:i + chunk_size] for i in range(0, len(market_ids), chunk_size)]
