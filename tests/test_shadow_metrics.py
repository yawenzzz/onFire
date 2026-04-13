import unittest

from polymarket_arb.shadow.metrics import ShadowMetrics


class ShadowMetricsTests(unittest.TestCase):
    def test_preview_success_rate_is_computed(self) -> None:
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
        self.assertEqual(metrics.preview_success_rate, 1.0)
        self.assertEqual(metrics.hedge_completion_rate_shadow, 0.9)
        self.assertEqual(metrics.false_positive_rate, 0.05)

    def test_zero_attempts_fail_closed_to_zero_rates(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id="s1",
            surface_id="polymarket-us",
            preview_successes=0,
            preview_attempts=0,
            hedge_completions=0,
            hedge_attempts=0,
            false_positives=0,
            candidate_count=0,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        self.assertEqual(metrics.preview_success_rate, 0.0)
        self.assertEqual(metrics.hedge_completion_rate_shadow, 0.0)
        self.assertEqual(metrics.false_positive_rate, 0.0)
