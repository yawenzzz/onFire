import unittest

from polymarket_arb.models.types import CandidateBasket, Leg, MarketState
from polymarket_arb.strategy.portfolio import portfolio_locked_capital, portfolio_total_pi_min


class PortfolioTests(unittest.TestCase):
    def _basket(self, pi_min: float, capital_efficiency: float) -> CandidateBasket:
        return CandidateBasket(
            group_id="g",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=True,
            zero_rebate_positive=True,
            pi_min_stress_usd=pi_min,
            hedge_completion_prob=0.99,
            capital_efficiency=capital_efficiency,
            legs=[Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a")],
        )

    def test_total_pi_min_sums_across_baskets(self) -> None:
        total = portfolio_total_pi_min([self._basket(1.0, 0.5), self._basket(2.0, 0.25)])
        self.assertEqual(total, 3.0)

    def test_locked_capital_uses_pi_over_efficiency(self) -> None:
        capital = portfolio_locked_capital([self._basket(1.0, 0.5)])
        self.assertEqual(capital, 2.0)
