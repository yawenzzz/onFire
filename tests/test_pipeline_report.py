import unittest

from polymarket_arb.app.pipeline import run_pipeline_report
from polymarket_arb.models.types import Leg, MarketState


class PipelineReportTests(unittest.TestCase):
    def test_pipeline_report_contains_verdict_and_metrics(self) -> None:
        report = run_pipeline_report(
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
        self.assertIn("surface_id", report)
        self.assertIn("preview_success_rate", report)
