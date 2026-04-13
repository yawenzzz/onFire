import unittest

from polymarket_arb.app.pipeline import run_pipeline
from polymarket_arb.models.types import Leg, MarketState


class PipelineTests(unittest.TestCase):
    def test_pipeline_returns_metrics_for_safe_inputs(self) -> None:
        metrics = run_pipeline(
            session_id="s1",
            surface_id="polymarket-us",
            outcome_count=2,
            ordered_thresholds=True,
            offset_relation=False,
            legs=[Leg("m1", "BUY", 0.4, MarketState.OPEN, True, 10, True, "a")],
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
        )
        self.assertEqual(metrics.surface_id, "polymarket-us")
        self.assertGreaterEqual(metrics.preview_success_rate, 0.0)
