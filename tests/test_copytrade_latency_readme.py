import unittest
from pathlib import Path


class CopytradeLatencyReadmeTests(unittest.TestCase):
    def test_latency_readme_mentions_report_script_and_core_metrics(self) -> None:
        text = Path("rust-copytrader/COPYTRADE_LATENCY.md").read_text()
        self.assertIn("run_rust_copytrade_latency_report.sh", text)
        self.assertIn("run_rust_follow_last_action_force_live_once.sh", text)
        self.assertIn("run_rust_minmax_follow_live.sh", text)
        self.assertIn("watch_elapsed_ms", text)
        self.assertIn("payload_prep_elapsed_ms", text)
        self.assertIn("submit_roundtrip_elapsed_ms", text)
        self.assertIn("price_gap_bps", text)
        self.assertIn("adverse_price_gap_bps", text)
