import unittest
from pathlib import Path


class AccountMonitorReadmeTests(unittest.TestCase):
    def test_account_monitor_readme_mentions_all_rust_entrypoints(self) -> None:
        text = Path("rust-copytrader/ACCOUNT_MONITOR.md").read_text()
        self.assertIn("run_rust_show_account_info.sh", text)
        self.assertIn("run_rust_account_monitor.sh", text)
        self.assertIn("run_rust_account_user_ws.sh", text)
        self.assertIn("run_copytrader_account_monitor", text)
        self.assertIn("run_copytrader_account_ws", text)
        self.assertIn("effective_funder_address", text)
        self.assertIn("signature_type", text)
        self.assertIn(".omx/account-monitor/latest.json", text)
