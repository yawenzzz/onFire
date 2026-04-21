import unittest
from pathlib import Path


class FollowLastActionForceLiveOnceScriptTests(unittest.TestCase):
    def test_force_live_follow_once_script_contains_force_submit_path(self) -> None:
        text = Path("scripts/run_rust_follow_last_action_force_live_once.sh").read_text()
        self.assertIn("--allow-live-submit", text)
        self.assertIn("--force-live-submit", text)
        self.assertIn("--override-usdc-size", text)
        self.assertIn("run_rust_watch_copy_leader_activity.sh", text)
        self.assertIn("run_rust_live_submit_gate.sh", text)
        self.assertIn("using cached latest activity", text)
        self.assertIn("last-submitted-tx.txt", text)
        self.assertNotIn("python3", text)
