import unittest

from polymarket_arb.config.schemas import LaunchGate


class LaunchGateTests(unittest.TestCase):
    def test_defaults_fail_closed(self) -> None:
        gate = LaunchGate()
        self.assertFalse(gate.launch_eligible())
        self.assertEqual(gate.posture(), "NO_TRADE")

    def test_launch_ready_requires_all_strict_conditions(self) -> None:
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
        self.assertTrue(gate.launch_eligible())
        self.assertEqual(gate.posture(), "LIVE_CAPABLE_READY")
