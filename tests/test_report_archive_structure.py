import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.shadow.report_archive import archive_report


class ReportArchiveStructureTests(unittest.TestCase):
    def test_archives_under_session_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            archived = archive_report(Path(tmp), session_id='s1', report={'verdict': 'LIVE_CAPABLE_READY'})
            self.assertIn('sessions', archived.parts)
            self.assertIn('s1', archived.parts)
            self.assertEqual(archived.name, 'certification-report.json')
            data = json.loads(archived.read_text())
            self.assertEqual(data['verdict'], 'LIVE_CAPABLE_READY')
