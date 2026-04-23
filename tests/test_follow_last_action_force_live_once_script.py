import unittest
from pathlib import Path


class FollowLastActionForceLiveOnceScriptTests(unittest.TestCase):
    def test_force_live_follow_once_script_contains_force_submit_path(self) -> None:
        text = Path("scripts/run_rust_follow_last_action_force_live_once.sh").read_text()
        self.assertIn("--allow-live-submit", text)
        self.assertIn("--force-live-submit", text)
        self.assertIn("--override-usdc-size", text)
        self.assertIn("--order-type", text)
        self.assertIn("run_rust_watch_copy_leader_activity.sh", text)
        self.assertIn("run_rust_live_submit_gate.sh", text)
        self.assertIn("run_rust_ctf_action.sh", text)
        self.assertIn("run_rust_public_positions_gate.sh", text)
        self.assertIn("run_rust_show_account_info.sh", text)
        self.assertIn('WATCH_ACTIVITY_TYPES="${WATCH_ACTIVITY_TYPES:-TRADE,MERGE,SPLIT}"', text)
        self.assertIn('FOLLOW_SHARE_DIVISOR="${FOLLOW_SHARE_DIVISOR:-10}"', text)
        self.assertIn('MIN_OPEN_SHARES="${MIN_OPEN_SHARES:-5}"', text)
        self.assertIn('POSITIONS_GATE_BIN_DEFAULT="${POSITIONS_GATE_BIN_DEFAULT:-$ROOT/scripts/run_rust_public_positions_gate.sh}"', text)
        self.assertIn('ACCOUNT_SNAPSHOT_BIN_DEFAULT="${ACCOUNT_SNAPSHOT_BIN_DEFAULT:-$ROOT/scripts/run_rust_show_account_info.sh}"', text)
        self.assertIn("leader_event_open_gate_status", text)
        self.assertIn("follower_current_asset_held", text)
        self.assertIn("follow_trigger_reason", text)
        self.assertIn("--follow-share-divisor", text)
        self.assertIn("follow_min_open_floor_applied", text)
        self.assertIn("submit_order_id", text)
        self.assertIn("submit_trade_ids", text)
        self.assertIn("using cached latest activity", text)
        self.assertIn("last-submitted-tx.txt", text)
        self.assertNotIn("127.0.0.1:7897", text)
        self.assertNotIn("python3", text)
