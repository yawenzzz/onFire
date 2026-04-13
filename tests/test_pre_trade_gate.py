import unittest

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.execution.pre_trade_gate import PreTradeGate
from polymarket_arb.models.types import CandidateBasket, Leg, MarketState


class PreTradeGateTests(unittest.TestCase):
    def _basket(self, *, preview_all_legs: bool = True, market_state_all_open: bool = True) -> CandidateBasket:
        leg = Leg(
            market_id="m1",
            side="BUY",
            price=0.5,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=preview_all_legs,
            clarification_hash="abc",
        )
        return CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=market_state_all_open,
            preview_all_legs=preview_all_legs,
            zero_rebate_positive=True,
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            legs=[leg],
        )

    def test_allows_only_when_launch_gate_and_basket_are_clean(self) -> None:
        gate = LaunchGate(
            surface_resolved=True,
            surface_id="polymarket-us",
            jurisdiction_eligible=True,
            market_state_all_open=True,
            preview_success_rate=1.0,
            invalid_tick_or_price_reject_rate=0.0,
            api_429_count=0,
            ambiguous_rule_trade_count=0,
            collateral_return_dependency_for_safety=0,
            hedge_completion_rate_shadow=0.995,
            false_positive_rate=0.02,
            shadow_window_days=14,
        )
        self.assertTrue(PreTradeGate().allow(self._basket(), gate))

    def test_blocks_when_basket_preview_is_not_clean(self) -> None:
        gate = LaunchGate(
            surface_resolved=True,
            surface_id="polymarket-us",
            jurisdiction_eligible=True,
            market_state_all_open=True,
            preview_success_rate=1.0,
            invalid_tick_or_price_reject_rate=0.0,
            api_429_count=0,
            ambiguous_rule_trade_count=0,
            collateral_return_dependency_for_safety=0,
            hedge_completion_rate_shadow=0.995,
            false_positive_rate=0.02,
            shadow_window_days=14,
        )
        self.assertFalse(PreTradeGate().allow(self._basket(preview_all_legs=False), gate))
