import unittest
from pathlib import Path


class CaptureShadowScriptTests(unittest.TestCase):
    def test_script_exists_and_mentions_jsonl(self) -> None:
        path = Path('scripts/run_capture_to_shadow_demo.sh')
        self.assertTrue(path.exists())
        text = path.read_text()
        self.assertIn('jsonl', text)
        self.assertIn('examples/live', text)
