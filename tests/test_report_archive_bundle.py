import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.shadow.report_archive import archive_bundle


class ReportArchiveBundleTests(unittest.TestCase):
    def test_archive_bundle_writes_report_summary_and_dashboard(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report_path, summary_path, dashboard_path = archive_bundle(
                Path(tmp),
                session_id='s1',
                report={'verdict': 'LIVE_CAPABLE_READY'},
                summary='mode=shadow verdict=LIVE_CAPABLE_READY',
                dashboard={'preview_success_rate': 1.0},
            )
            self.assertTrue(report_path.exists())
            self.assertTrue(summary_path.exists())
            self.assertTrue(dashboard_path.exists())
            self.assertEqual(json.loads(report_path.read_text())['verdict'], 'LIVE_CAPABLE_READY')
            self.assertIn('mode=shadow', summary_path.read_text())
            self.assertEqual(json.loads(dashboard_path.read_text())['preview_success_rate'], 1.0)
