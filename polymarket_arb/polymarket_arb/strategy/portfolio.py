from __future__ import annotations

from polymarket_arb.models.types import CandidateBasket


def portfolio_total_pi_min(baskets: list[CandidateBasket]) -> float:
    return sum(basket.pi_min_stress_usd for basket in baskets)


def portfolio_locked_capital(baskets: list[CandidateBasket]) -> float:
    total = 0.0
    for basket in baskets:
        if basket.capital_efficiency <= 0:
            continue
        total += basket.pi_min_stress_usd / basket.capital_efficiency
    return total
