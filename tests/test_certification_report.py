import unittest

from polymarket_arb.shadow.certification_report import build_certification_report
from polymarket_arb.shadow.metrics import ShadowMetrics


class CertificationReportTests(unittest.TestCase):
    def test_builds_report_with_verdict_and_metrics(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id="s1",
            surface_id="polymarket-us",
            preview_successes=10,
            preview_attempts=10,
            hedge_completions=10,
            hedge_attempts=10,
            false_positives=1,
            candidate_count=100,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        report = build_certification_report(verdict="LIVE_CAPABLE_READY", metrics=metrics)
        self.assertEqual(report["verdict"], "LIVE_CAPABLE_READY")
        self.assertEqual(report["surface_id"], "polymarket-us")
        self.assertIn("preview_success_rate", report)
