import unittest

from polymarket_arb.app.cli import build_cli_summary
from polymarket_arb.models.types import Leg, MarketState
from polymarket_arb.shadow.session_runner import run_shadow_session


class SessionRunnerTests(unittest.TestCase):
    def test_run_shadow_session_returns_report_and_summary(self) -> None:
        report, summary = run_shadow_session(
            session_id="s1",
            surface_id="polymarket-us",
            outcome_count=2,
            ordered_thresholds=True,
            offset_relation=False,
            legs=[Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a")],
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            surface_resolved=True,
            jurisdiction_eligible=True,
        )
        self.assertIn("verdict", report)
        self.assertEqual(summary, build_cli_summary(mode="shadow", verdict=report["verdict"]))
