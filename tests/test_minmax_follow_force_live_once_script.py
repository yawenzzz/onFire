import unittest
from pathlib import Path


class MinmaxFollowForceLiveOnceScriptTests(unittest.TestCase):
    def test_force_live_once_launcher_passes_force_flags_without_python_helpers(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_force_live_once.sh").read_text()
        self.assertIn("--allow-live-submit", text)
        self.assertIn("--force-live-submit", text)
        self.assertIn("--ignore-seen-tx", text)
        self.assertIn("--min-open-usdc", text)
        self.assertIn("--max-open-usdc", text)
        self.assertNotIn("127.0.0.1:7897", text)
        self.assertNotIn("PYTHON_BIN", text)
        self.assertNotIn("python3", text)
        self.assertNotIn("run_refresh_account_snapshot.sh", text)
        self.assertNotIn("run_user_channel_probe.sh", text)
