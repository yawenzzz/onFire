import unittest
from pathlib import Path


class MinmaxFollowLiveSubmitScriptTests(unittest.TestCase):
    def test_live_submit_launcher_reuses_force_follow_once_path_for_continuous_submit(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_submit.sh").read_text()
        self.assertIn('FORCE_FOLLOW_ONCE_BIN="${FORCE_FOLLOW_ONCE_BIN:-$ROOT/scripts/run_rust_follow_last_action_force_live_once.sh}"', text)
        self.assertIn('FOLLOW_FOREVER="${FOLLOW_FOREVER:-1}"', text)
        self.assertIn('RESTART_ON_FAILURE="${RESTART_ON_FAILURE:-1}"', text)
        self.assertIn('LOOP_DELAY_SECONDS="${LOOP_DELAY_SECONDS:-1}"', text)
        self.assertIn('REQUIRE_NEW_ACTIVITY=1 bash "$FORCE_FOLLOW_ONCE_BIN" "$@"', text)
        self.assertNotIn('LIVE_FOLLOW_BIN="${LIVE_FOLLOW_BIN', text)
