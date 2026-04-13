import unittest

from polymarket_arb.strategy.cross_market_model import cross_market_equivalence_score


class CrossMarketModelTests(unittest.TestCase):
    def test_rejects_non_deterministic_equivalence(self) -> None:
        result = cross_market_equivalence_score(rho_equiv=0.8, pi_min_stress_usd=1.0)
        self.assertFalse(result["allowed"])
        self.assertEqual(result["reason"], "non-deterministic equivalence")

    def test_accepts_deterministic_equivalence(self) -> None:
        result = cross_market_equivalence_score(rho_equiv=1.0, pi_min_stress_usd=1.2)
        self.assertTrue(result["allowed"])
        self.assertEqual(result["adjusted_edge"], 1.2)
