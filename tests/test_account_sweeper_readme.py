import unittest
from pathlib import Path


class AccountSweeperReadmeTests(unittest.TestCase):
    def test_account_sweeper_docs_cover_wrapper_bin_and_log_path(self) -> None:
        text = Path("rust-copytrader/ACCOUNT_SWEEPER.md").read_text()
        self.assertIn("run_rust_account_sweeper.sh", text)
        self.assertIn("run_copytrader_account_sweeper", text)
        self.assertIn("logs/account-sweeper/account-sweeper.log", text)
        self.assertIn("ALLOW_LIVE_SUBMIT=0", text)
        self.assertIn("independent_of_main_follow=true", text)
        self.assertIn("redeem_neg_risk", text)
