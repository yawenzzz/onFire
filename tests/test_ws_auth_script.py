import unittest
from pathlib import Path


class WSAuthScriptTests(unittest.TestCase):
    def test_script_exists_and_mentions_env_loading(self) -> None:
        text = Path('scripts/run_realtime_probe_auth.sh').read_text()
        self.assertIn('PM_ACCESS_KEY', text)
        self.assertIn('polymarket_arb.app.realtime_probe_cli', text)
