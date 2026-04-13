import unittest

from polymarket_arb.strategy.parity_model import long_basket_edge


class ParityModelTests(unittest.TestCase):
    def test_long_basket_edge_is_positive_when_sum_of_prices_is_below_payout(self) -> None:
        edge = long_basket_edge(prices=[0.30, 0.25, 0.20], quantity=1.0, fee_cost=0.01, slippage_cost=0.01)
        self.assertAlmostEqual(edge, 0.23)

    def test_requires_at_least_two_outcomes(self) -> None:
        with self.assertRaises(ValueError):
            long_basket_edge(prices=[0.4], quantity=1.0)
