import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.live_bundle_cli import main


class LiveBundleCliTests(unittest.TestCase):
    def test_main_in_demo_mode_writes_archive_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            archive_root = Path(tmp) / 'archive'
            capture_path = Path(tmp) / 'capture.jsonl'
            code = main([
                '--archive-root', str(archive_root),
                '--capture-output', str(capture_path),
                '--session-id', 's1',
                '--surface-id', 'polymarket-us',
                '--demo',
                '--limit', '1',
            ])
            self.assertEqual(code, 0)
            report = json.loads((archive_root / 'sessions' / 's1' / 'certification-report.json').read_text())
            self.assertEqual(report['snapshot_count'], 1)
            self.assertTrue(capture_path.exists())
