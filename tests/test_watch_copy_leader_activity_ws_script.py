import unittest
from pathlib import Path


class WatchCopyLeaderActivityWsScriptTests(unittest.TestCase):
    def test_ws_watch_script_runs_new_ws_bin(self) -> None:
        text = Path("scripts/run_rust_watch_copy_leader_activity_ws.sh").read_text()
        self.assertIn('run --bin watch_copy_leader_activity_ws', text)
        self.assertNotIn("127.0.0.1:7897", text)
