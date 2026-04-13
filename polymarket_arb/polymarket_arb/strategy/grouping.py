from __future__ import annotations

from polymarket_arb.models.types import CandidateBasket, Leg
from polymarket_arb.rules.template_whitelist import is_whitelisted


def build_candidate_basket(
    group_id: str,
    surface_id: str,
    template_type: str,
    legs: list[Leg],
    pi_min_stress_usd: float,
    hedge_completion_prob: float,
    capital_efficiency: float,
) -> CandidateBasket:
    if not legs:
        raise ValueError("candidate group must contain at least one leg")
    if not is_whitelisted(template_type):
        raise ValueError("template type is not whitelisted")

    return CandidateBasket(
        group_id=group_id,
        template_type=template_type,
        surface_id=surface_id,
        rule_hash_unchanged=True,
        clarification_hash_unchanged=True,
        market_state_all_open=all(leg.market_state == leg.market_state.OPEN for leg in legs),
        preview_all_legs=all(leg.preview_ok for leg in legs),
        zero_rebate_positive=pi_min_stress_usd > 0,
        pi_min_stress_usd=pi_min_stress_usd,
        hedge_completion_prob=hedge_completion_prob,
        capital_efficiency=capital_efficiency,
        legs=legs,
    )
