import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.capture_rotate import rotate_capture_path


class CaptureRotateTests(unittest.TestCase):
    def test_rotate_capture_path_puts_file_under_session_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = rotate_capture_path(Path(tmp), session_id='s1')
            self.assertIn('sessions', path.parts)
            self.assertIn('s1', path.parts)
            self.assertTrue(path.name.endswith('.jsonl'))
