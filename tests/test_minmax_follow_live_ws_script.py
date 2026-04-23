import unittest
from pathlib import Path


class MinmaxFollowLiveWsScriptTests(unittest.TestCase):
    def test_live_ws_script_switches_watch_bin_to_ws_watcher(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_ws.sh").read_text()
        self.assertIn('WATCH_BIN_DEFAULT="$ROOT/scripts/run_rust_watch_copy_leader_activity_ws.sh"', text)
        self.assertIn('run_rust_minmax_follow_live.sh', text)

    def test_live_submit_ws_script_switches_watch_bin_to_ws_watcher(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_submit_ws.sh").read_text()
        self.assertIn('WATCH_BIN_DEFAULT="$ROOT/scripts/run_rust_watch_copy_leader_activity_ws.sh"', text)
        self.assertIn('run_rust_minmax_follow_live_submit.sh', text)
