import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_bundle import run_capture_bundle


class CaptureBundleCommandTests(unittest.TestCase):
    def test_capture_bundle_writes_archive_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            capture = Path(tmp) / 'capture.jsonl'
            capture.write_text('{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n')
            archive_root = Path(tmp) / 'archive'
            report, summary, dashboard = run_capture_bundle(
                capture_path=capture,
                archive_root=archive_root,
                session_id='s1',
                surface_id='polymarket-us',
            )
            self.assertTrue(report.exists())
            self.assertTrue(summary.exists())
            self.assertTrue(dashboard.exists())
            self.assertEqual(json.loads(report.read_text())['snapshot_count'], 1)
