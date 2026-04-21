import unittest
from pathlib import Path


class MinmaxFollowLiveScriptTests(unittest.TestCase):
    def test_live_script_defaults_to_safe_preview_mode_and_is_rust_only(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live.sh").read_text()
        self.assertNotIn("PYTHON_BIN", text)
        self.assertNotIn("python3", text)
        self.assertNotIn("run_refresh_account_snapshot.sh", text)
        self.assertNotIn("run_user_channel_probe.sh", text)
        self.assertIn('FOLLOW_FOREVER="${FOLLOW_FOREVER:-0}"', text)
        self.assertIn('AUTO_SUBMIT="${AUTO_SUBMIT:-0}"', text)
        self.assertIn('MIN_OPEN_USDC="${MIN_OPEN_USDC:-0.1}"', text)
        self.assertIn('MAX_OPEN_USDC="${MAX_OPEN_USDC:-10}"', text)
        self.assertIn('MAX_TOTAL_EXPOSURE_USDC="${MAX_TOTAL_EXPOSURE_USDC:-100}"', text)
        self.assertIn('MAX_ORDER_USDC="${MAX_ORDER_USDC:-10}"', text)
        self.assertIn('RESTART_ON_FAILURE="${RESTART_ON_FAILURE:-1}"', text)
        self.assertIn('MAX_RESTARTS="${MAX_RESTARTS:-20}"', text)
        self.assertIn('RESTART_DELAY_SECONDS="${RESTART_DELAY_SECONDS:-5}"', text)
        self.assertNotIn("127.0.0.1:7897", text)
        self.assertIn('--max-total-exposure-usdc "$MAX_TOTAL_EXPOSURE_USDC"', text)
        self.assertIn('--max-order-usdc "$MAX_ORDER_USDC"', text)
        self.assertIn('--account-snapshot "$ACCOUNT_SNAPSHOT_PATH"', text)
        self.assertIn('while true; do', text)
        self.assertIn('run_rust_minmax_follow.sh failed', text)
