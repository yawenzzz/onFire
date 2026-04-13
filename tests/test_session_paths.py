import unittest
from pathlib import Path

from polymarket_arb.data.session_paths import session_root, session_file


class SessionPathsTests(unittest.TestCase):
    def test_session_root_is_namespaced(self) -> None:
        root = session_root(Path('/tmp/x'), 's1')
        self.assertEqual(root, Path('/tmp/x') / 'sessions' / 's1')

    def test_session_file_joins_filename(self) -> None:
        path = session_file(Path('/tmp/x'), 's1', 'capture.jsonl')
        self.assertEqual(path, Path('/tmp/x') / 'sessions' / 's1' / 'capture.jsonl')
