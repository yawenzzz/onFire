import unittest
from pathlib import Path


class ClobAuthDocTests(unittest.TestCase):
    def test_doc_mentions_api_key_secret_passphrase(self) -> None:
        text = Path('docs/clob-auth-notes.md').read_text()
        self.assertIn('api_key', text)
        self.assertIn('api_secret', text)
        self.assertIn('api_passphrase', text)
