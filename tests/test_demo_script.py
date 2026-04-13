import unittest
from pathlib import Path


class DemoScriptTests(unittest.TestCase):
    def test_demo_script_exists(self) -> None:
        path = Path('scripts/run_shadow_demo.sh')
        self.assertTrue(path.exists())
        self.assertIn('examples/shadow-input.json', path.read_text())
