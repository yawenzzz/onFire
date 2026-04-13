from __future__ import annotations


def cross_market_equivalence_score(rho_equiv: float, pi_min_stress_usd: float) -> dict:
    if rho_equiv != 1.0:
        return {
            "allowed": False,
            "reason": "non-deterministic equivalence",
            "adjusted_edge": float("-inf"),
        }
    return {
        "allowed": True,
        "reason": None,
        "adjusted_edge": pi_min_stress_usd,
    }
