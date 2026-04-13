import unittest

from polymarket_arb.shadow.dashboard_schema import build_dashboard_payload
from polymarket_arb.shadow.metrics import ShadowMetrics


class DashboardSchemaTests(unittest.TestCase):
    def test_builds_dashboard_payload(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id="s1",
            surface_id="polymarket-us",
            preview_successes=10,
            preview_attempts=10,
            hedge_completions=9,
            hedge_attempts=10,
            false_positives=1,
            candidate_count=20,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        payload = build_dashboard_payload(metrics)
        self.assertEqual(payload["session_id"], "s1")
        self.assertEqual(payload["surface_id"], "polymarket-us")
        self.assertIn("preview_success_rate", payload)
