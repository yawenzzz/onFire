import unittest
from pathlib import Path


class MainReadmeTests(unittest.TestCase):
    def test_readme_lists_current_script_entrypoints_and_key_outputs(self) -> None:
        text = Path('rust-copytrader/README.md').read_text()
        for needle in [
            'run_rust_watch_copy_leader_activity.sh',
            'run_rust_watch_copy_leader_activity_ws.sh',
            'run_rust_minmax_follow.sh',
            'run_rust_minmax_follow_live.sh',
            'run_rust_minmax_follow_live_ws.sh',
            'run_rust_minmax_follow_live_submit.sh',
            'run_rust_minmax_follow_live_submit_ws.sh',
            'run_rust_minmax_follow_live_submit_once.sh',
            'run_rust_minmax_follow_force_live_once.sh',
            'run_rust_follow_last_action_force_live_once.sh',
            'run_rust_live_submit_gate.sh',
            'run_rust_public_positions_gate.sh',
            'run_rust_ctf_action.sh',
            'run_rust_copytrade_latency_report.sh',
            'run_rust_copytrade_fill_latency_logger.sh',
            'run_rust_minmax_follow_live_submit_latency.sh',
            'run_rust_show_account_info.sh',
            'run_rust_account_monitor.sh',
            'run_rust_account_user_ws.sh',
            'logs/copytrade-fill-latency/<leader_wallet>/fills.log',
            'leader 动作发生时间 -> follower 真实成交时间',
            'PRIVATE_KEY',
            'CLOB_SECRET',
            'authenticated websocket',
            '主跟单只负责跟单',
            '`[info]: ...`',
            'corr=trade_id|order_id|tx_hash',
        ]:
            self.assertIn(needle, text)
