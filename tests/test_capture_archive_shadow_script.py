import unittest
from pathlib import Path


class CaptureArchiveShadowScriptTests(unittest.TestCase):
    def test_script_exists_and_mentions_capture_jsonl(self) -> None:
        path = Path('scripts/run_capture_archive_shadow.sh')
        self.assertTrue(path.exists())
        text = path.read_text()
        self.assertIn('capture.jsonl', text)
        self.assertIn('certification-report.json', text)
