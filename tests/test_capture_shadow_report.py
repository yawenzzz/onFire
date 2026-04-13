import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.shadow.capture_report import build_capture_shadow_report


class CaptureShadowReportTests(unittest.TestCase):
    def test_builds_fail_closed_report_from_jsonl(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'capture.jsonl'
            path.write_text(
                '{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n'
                '{"market_id":"m2","market_state":"OPEN","best_bid":0.3,"best_ask":0.6}\n'
            )
            report = build_capture_shadow_report(path, session_id='s1', surface_id='polymarket-us')
            self.assertEqual(report['verdict'], 'CERTIFICATION_INCOMPLETE')
            self.assertEqual(report['snapshot_count'], 2)
