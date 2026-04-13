from __future__ import annotations


def directional_pair_payoff(
    lower_yes_price: float,
    higher_no_price: float,
    quantity: float,
    fee_cost: float = 0.0,
    slippage_cost: float = 0.0,
) -> dict:
    if quantity <= 0:
        raise ValueError("quantity must be positive")

    entry_cost = (lower_yes_price + higher_no_price) * quantity + fee_cost + slippage_cost
    scenario_edges = {
        "below_lower": quantity - entry_cost,
        "between": (2 * quantity) - entry_cost,
        "above_higher": quantity - entry_cost,
    }
    return {
        "entry_cost": entry_cost,
        "scenario_edges": scenario_edges,
        "worst_case_edge": min(scenario_edges.values()),
    }
