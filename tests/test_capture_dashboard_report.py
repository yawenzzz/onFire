import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.shadow.capture_dashboard import build_capture_dashboard


class CaptureDashboardReportTests(unittest.TestCase):
    def test_builds_dashboard_from_capture_jsonl(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'capture.jsonl'
            path.write_text(
                '{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n'
            )
            dashboard = build_capture_dashboard(path)
            self.assertEqual(dashboard['snapshot_count'], 1)
            self.assertIn('preview_success_rate', dashboard)
