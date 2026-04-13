import unittest
from pathlib import Path


class WSAuthDemoDocTests(unittest.TestCase):
    def test_doc_exists_and_mentions_env_vars(self) -> None:
        text = Path('docs/ws-auth-demo.md').read_text()
        self.assertIn('PM_ACCESS_KEY', text)
        self.assertIn('PM_SIGNATURE', text)
        self.assertIn('PM_TIMESTAMP', text)
