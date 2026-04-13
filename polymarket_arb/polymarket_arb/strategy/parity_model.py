from __future__ import annotations


def long_basket_edge(
    prices: list[float],
    quantity: float,
    fee_cost: float = 0.0,
    slippage_cost: float = 0.0,
) -> float:
    if len(prices) < 2:
        raise ValueError("at least two outcomes are required for parity arbitrage")
    payout = quantity
    total_cost = sum(prices) * quantity + fee_cost + slippage_cost
    return payout - total_cost
