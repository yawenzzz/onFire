import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.sample_to_shadow_bundle import run_sample_to_shadow_bundle


class SampleToShadowBundleTests(unittest.TestCase):
    def test_runs_bundle_from_market_sample_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            sample = Path(tmp) / 'capture.jsonl'
            sample.write_text('{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n')
            archive_root = Path(tmp) / 'archive'
            report_path, summary_path, dashboard_path = run_sample_to_shadow_bundle(
                capture_path=sample,
                archive_root=archive_root,
                session_id='s1',
                surface_id='polymarket-us',
            )
            self.assertEqual(json.loads(report_path.read_text())['snapshot_count'], 1)
            self.assertIn('mode=shadow', summary_path.read_text())
            self.assertIn('snapshot_count', json.loads(dashboard_path.read_text()))
