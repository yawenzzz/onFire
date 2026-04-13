import unittest

from polymarket_arb.models.types import CandidateBasket, Leg, MarketState
from polymarket_arb.shadow.replay_runner import ReplayRunner


class ReplayRunnerTests(unittest.TestCase):
    def test_replay_runner_returns_shadow_metrics(self) -> None:
        basket = CandidateBasket(
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
            legs=[Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a")],
        )
        metrics = ReplayRunner().run(session_id="s1", surface_id="polymarket-us", baskets=[basket])
        self.assertEqual(metrics.surface_id, "polymarket-us")
        self.assertEqual(metrics.preview_success_rate, 1.0)
