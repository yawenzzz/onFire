import unittest
from pathlib import Path


class CliDemoDocTests(unittest.TestCase):
    def test_demo_doc_exists_and_mentions_python_module_invocation(self) -> None:
        path = Path('docs/cli-demo.md')
        self.assertTrue(path.exists())
        text = path.read_text()
        self.assertIn('python -m polymarket_arb.app.entrypoint', text)
