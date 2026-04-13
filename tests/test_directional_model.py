import unittest

from polymarket_arb.strategy.directional_model import directional_pair_payoff


class DirectionalModelTests(unittest.TestCase):
    def test_lower_yes_and_higher_no_has_bounded_worst_case(self) -> None:
        payoff = directional_pair_payoff(
            lower_yes_price=0.62,
            higher_no_price=0.55,
            quantity=1.0,
            fee_cost=0.01,
            slippage_cost=0.01,
        )
        self.assertAlmostEqual(payoff["worst_case_edge"], -0.19)
        self.assertIn("below_lower", payoff["scenario_edges"])
        self.assertIn("between", payoff["scenario_edges"])
        self.assertIn("above_higher", payoff["scenario_edges"])

    def test_requires_positive_quantity(self) -> None:
        with self.assertRaises(ValueError):
            directional_pair_payoff(0.6, 0.5, quantity=0)
