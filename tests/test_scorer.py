import unittest

from polymarket_arb.models.types import CandidateBasket, Leg, MarketState
from polymarket_arb.rules.drift_guard import DriftDecision
from polymarket_arb.strategy.scorer import RobustScorer


class RobustScorerTests(unittest.TestCase):
    def _leg(self) -> Leg:
        return Leg(
            market_id="m1",
            side="BUY",
            price=0.5,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=True,
            clarification_hash="abc",
        )

    def test_returns_reject_when_hard_gate_fails(self) -> None:
        basket = CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=False,
            zero_rebate_positive=True,
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.1,
            legs=[self._leg()],
        )
        score = RobustScorer().score(basket)
        self.assertTrue(score.rejected)
        self.assertEqual(score.rank_score, float("-inf"))

    def test_rejects_when_drift_guard_blocks_candidate(self) -> None:
        basket = CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=True,
            zero_rebate_positive=True,
            pi_min_stress_usd=2.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            legs=[self._leg()],
        )
        score = RobustScorer().score(
            basket,
            drift=DriftDecision(allowed=False, reasons=["rule hash drift"]),
        )
        self.assertTrue(score.rejected)
        self.assertIn("rule hash drift", score.reject_reasons)

    def test_scores_positive_candidate(self) -> None:
        basket = CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=True,
            zero_rebate_positive=True,
            pi_min_stress_usd=2.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            ambiguity_penalty=0.0,
            ops_penalty=0.1,
            legs=[self._leg()],
        )
        score = RobustScorer().score(basket)
        self.assertFalse(score.rejected)
        self.assertGreater(score.score_raw, 0)
        self.assertGreater(score.rank_score, 0)
