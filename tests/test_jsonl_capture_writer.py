import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.jsonl_capture import append_jsonl


class JsonlCaptureWriterTests(unittest.TestCase):
    def test_appends_json_lines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'capture.jsonl'
            append_jsonl(path, {'a': 1})
            append_jsonl(path, {'b': 2})
            lines = path.read_text().strip().splitlines()
            self.assertEqual(json.loads(lines[0])['a'], 1)
            self.assertEqual(json.loads(lines[1])['b'], 2)
