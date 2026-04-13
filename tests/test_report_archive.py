import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.shadow.report_archive import archive_report


class ReportArchiveTests(unittest.TestCase):
    def test_archives_report_to_session_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            archived = archive_report(Path(tmp), session_id='s1', report={"verdict": "CERTIFICATION_INCOMPLETE"})
            self.assertTrue(archived.exists())
            data = json.loads(archived.read_text())
            self.assertEqual(data['verdict'], 'CERTIFICATION_INCOMPLETE')
