import unittest
from pathlib import Path


class LiveBundleScriptTests(unittest.TestCase):
    def test_script_exists_and_mentions_live_bundle_cli(self) -> None:
        path = Path('scripts/run_live_bundle_demo.sh')
        self.assertTrue(path.exists())
        text = path.read_text()
        self.assertIn('polymarket_arb.app.live_bundle_cli', text)
