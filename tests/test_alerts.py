import unittest

from polymarket_arb.ops.alerts import build_redline_alerts
from polymarket_arb.shadow.metrics import ShadowMetrics


class AlertsTests(unittest.TestCase):
    def test_emits_alert_on_preview_drop(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id="s1",
            surface_id="polymarket-us",
            preview_successes=9,
            preview_attempts=10,
            hedge_completions=10,
            hedge_attempts=10,
            false_positives=0,
            candidate_count=100,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        alerts = build_redline_alerts(metrics)
        self.assertIn("preview_success_rate < 1.0", alerts)

    def test_no_alerts_when_metrics_are_clean(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id="s1",
            surface_id="polymarket-us",
            preview_successes=10,
            preview_attempts=10,
            hedge_completions=10,
            hedge_attempts=10,
            false_positives=0,
            candidate_count=100,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        alerts = build_redline_alerts(metrics)
        self.assertEqual(alerts, [])
