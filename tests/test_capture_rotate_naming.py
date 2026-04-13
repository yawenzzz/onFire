import unittest
from pathlib import Path

from polymarket_arb.data.capture_rotate_naming import rotated_capture_name


class CaptureRotateNamingTests(unittest.TestCase):
    def test_rotated_name_contains_prefix_and_suffix(self) -> None:
        name = rotated_capture_name(prefix='capture', suffix='jsonl', sequence=3)
        self.assertTrue(name.startswith('capture-0003'))
        self.assertTrue(name.endswith('.jsonl'))
