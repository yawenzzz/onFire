import unittest
from pathlib import Path


class ReadmeDevGuideTests(unittest.TestCase):
    def test_readme_contains_dev_quickstart(self) -> None:
        text = Path('polymarket_arb/README.md').read_text()
        self.assertIn('Development Quickstart', text)
        self.assertIn('make test', text)
        self.assertIn('--input-file', text)
