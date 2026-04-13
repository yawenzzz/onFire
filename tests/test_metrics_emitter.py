import unittest

from polymarket_arb.ops.metrics_emitter import build_metrics_payload
from polymarket_arb.shadow.metrics import ShadowMetrics


class MetricsEmitterTests(unittest.TestCase):
    def test_builds_metrics_payload(self) -> None:
        metrics = ShadowMetrics.from_counts(
            session_id='s1',
            surface_id='polymarket-us',
            preview_successes=10,
            preview_attempts=10,
            hedge_completions=9,
            hedge_attempts=10,
            false_positives=1,
            candidate_count=20,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
        payload = build_metrics_payload(metrics, reconnect_count=2, parse_error_count=1)
        self.assertEqual(payload['session_id'], 's1')
        self.assertEqual(payload['reconnect_count'], 2)
        self.assertEqual(payload['parse_error_count'], 1)
