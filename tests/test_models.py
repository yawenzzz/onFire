import unittest

from polymarket_arb.models.types import CandidateBasket, Leg, MarketState


class ModelTests(unittest.TestCase):
    def test_leg_tradeable_requires_open_tick_depth_preview_and_price_bounds(self) -> None:
        leg = Leg(
            market_id="m1",
            side="BUY",
            price=0.5,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=True,
            clarification_hash="abc",
        )
        self.assertTrue(leg.is_tradeable())

    def test_candidate_requires_structurally_safe_legs(self) -> None:
        leg = Leg(
            market_id="m1",
            side="BUY",
            price=0.5,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=True,
            clarification_hash="abc",
        )
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
            capital_efficiency=0.1,
            legs=[leg],
        )
        self.assertTrue(basket.is_structurally_safe())
