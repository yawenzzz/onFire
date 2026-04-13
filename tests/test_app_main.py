import unittest

from polymarket_arb.app.main import run_app
from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.venue.surface_gate import SurfaceGate


class AppMainTests(unittest.TestCase):
    def test_app_starts_in_no_trade_when_surface_unresolved(self) -> None:
        posture = run_app(SurfaceGate(), LaunchGate())
        self.assertEqual(posture, "NO_TRADE")

    def test_app_can_report_live_capable_ready_when_all_gates_green(self) -> None:
        posture = run_app(
            SurfaceGate(surface_id="polymarket-us", jurisdiction_eligible=True),
            LaunchGate(
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
            ),
        )
        self.assertEqual(posture, "LIVE_CAPABLE_READY")
