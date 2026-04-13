import unittest

from polymarket_arb.models.types import Leg, MarketState
from polymarket_arb.strategy.grouping import build_candidate_basket


class GroupingTests(unittest.TestCase):
    def _leg(self, market_id: str) -> Leg:
        return Leg(
            market_id=market_id,
            side="BUY",
            price=0.25,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=True,
            clarification_hash="hash-a",
        )

    def test_builds_candidate_for_whitelisted_template(self) -> None:
        basket = build_candidate_basket(
            group_id="g1",
            surface_id="polymarket-us",
            template_type="exhaustive_set",
            legs=[self._leg("m1"), self._leg("m2")],
            pi_min_stress_usd=1.5,
            hedge_completion_prob=0.99,
            capital_efficiency=0.2,
        )
        self.assertEqual(basket.template_type, "exhaustive_set")
        self.assertTrue(basket.is_structurally_safe())

    def test_rejects_empty_leg_groups(self) -> None:
        with self.assertRaises(ValueError):
            build_candidate_basket(
                group_id="g1",
                surface_id="polymarket-us",
                template_type="exhaustive_set",
                legs=[],
                pi_min_stress_usd=1.0,
                hedge_completion_prob=0.99,
                capital_efficiency=0.1,
            )
