import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_bundle_cli import main


class CaptureBundleCliTests(unittest.TestCase):
    def test_main_builds_archive_from_capture_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            capture = Path(tmp) / 'capture.jsonl'
            capture.write_text('{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n')
            archive_root = Path(tmp) / 'archive'
            code = main([
                '--capture-file', str(capture),
                '--archive-root', str(archive_root),
                '--session-id', 's1',
                '--surface-id', 'polymarket-us',
            ])
            self.assertEqual(code, 0)
            report = json.loads((archive_root / 'sessions' / 's1' / 'certification-report.json').read_text())
            self.assertEqual(report['snapshot_count'], 1)
