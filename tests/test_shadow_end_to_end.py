import unittest

from polymarket_arb.app.entrypoint import run_shadow_entrypoint
from polymarket_arb.models.types import Leg, MarketState


class ShadowEndToEndTests(unittest.TestCase):
    def test_shadow_run_to_summary_flow(self) -> None:
        summary = run_shadow_entrypoint(
            session_id="s1",
            surface_id="polymarket-us",
            outcome_count=2,
            ordered_thresholds=True,
            offset_relation=False,
            legs=[Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a")],
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            surface_resolved=True,
            jurisdiction_eligible=True,
        )
        self.assertTrue(summary.startswith("mode=shadow verdict="))
