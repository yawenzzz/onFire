import unittest
from pathlib import Path


class MinmaxFollowLiveSubmitOnceScriptTests(unittest.TestCase):
    def test_live_submit_once_launcher_reuses_force_follow_once_path(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_submit_once.sh").read_text()
        self.assertIn('FORCE_FOLLOW_ONCE_BIN="${FORCE_FOLLOW_ONCE_BIN:-$ROOT/scripts/run_rust_follow_last_action_force_live_once.sh}"', text)
        self.assertIn('REQUIRE_NEW_ACTIVITY=1 bash "$FORCE_FOLLOW_ONCE_BIN" "$@"', text)
        self.assertNotIn('LIVE_FOLLOW_BIN="${LIVE_FOLLOW_BIN', text)
