import unittest

from polymarket_arb.execution.reconciliation import ReconciliationResult, Reconciler
from polymarket_arb.models.types import CandidateBasket, Leg, MarketState


class ReconciliationLayerTests(unittest.TestCase):
    def _basket(self) -> CandidateBasket:
        legs = [
            Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a"),
            Leg("m2", "BUY", 0.3, MarketState.OPEN, True, 10, True, "a"),
        ]
        return CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=True,
            zero_rebate_positive=True,
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            legs=legs,
        )

    def test_matches_when_expected_and_actual_markets_align(self) -> None:
        reconciler = Reconciler()
        result = reconciler.sync(self._basket(), filled_market_ids=["m1", "m2"])
        self.assertIsInstance(result, ReconciliationResult)
        self.assertTrue(result.matched)
        self.assertEqual(result.missing_market_ids, [])
        self.assertEqual(result.unexpected_market_ids, [])
        self.assertTrue(reconciler.matched())

    def test_reports_missing_and_unexpected_markets(self) -> None:
        reconciler = Reconciler()
        result = reconciler.sync(self._basket(), filled_market_ids=["m1", "m3"])
        self.assertFalse(result.matched)
        self.assertEqual(result.missing_market_ids, ["m2"])
        self.assertEqual(result.unexpected_market_ids, ["m3"])
        self.assertFalse(reconciler.matched())
