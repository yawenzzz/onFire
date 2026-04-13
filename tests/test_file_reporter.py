import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.file_reporter import write_json_report


class FileReporterTests(unittest.TestCase):
    def test_writes_json_report_to_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'report.json'
            write_json_report(path, {"verdict": "CERTIFICATION_INCOMPLETE"})
            data = json.loads(path.read_text())
            self.assertEqual(data["verdict"], "CERTIFICATION_INCOMPLETE")
