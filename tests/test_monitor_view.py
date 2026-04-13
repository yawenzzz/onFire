import unittest

from polymarket_arb.monitoring.models import EventScanResult, MonitorSnapshot
from polymarket_arb.monitoring.view import render_monitor_lines


class MonitorViewTests(unittest.TestCase):
    def test_render_monitor_lines_includes_top_candidate_and_rejection_summary(self) -> None:
        snapshot = MonitorSnapshot(
            iteration=3,
            event_count=20,
            complete_event_count=4,
            candidate_count=1,
            best_gross_edge=0.041,
            best_cost_adjusted_edge=0.011,
            scan_duration_seconds=1.42,
            candidates=[
                EventScanResult(
                    slug="fannie-mae-ipo-closing-market-cap",
                    title="Fannie Mae IPO Closing Market Cap",
                    template_type="market_cap_buckets",
                    is_complete=True,
                    open_market_count=7,
                    total_market_count=7,
                    sum_ask=0.959,
                    gross_edge=0.041,
                    cost_adjusted_edge=0.011,
                    rejection_reason=None,
                    questions=[],
                )
            ],
            rejections=[],
            rejection_counts={"overlapping_thresholds": 9, "open_candidate_set": 7},
            data_source="gamma-api",
            status="ok",
        )

        lines = render_monitor_lines(snapshot, width=120, height=20)
        text = "\n".join(lines)

        self.assertIn("Iteration 3", text)
        self.assertIn("gamma-api", text)
        self.assertIn("Fannie Mae IPO Closing Market Cap", text)
        self.assertIn("0.041", text)
        self.assertIn("overlapping_thresholds", text)
        self.assertIn("open_candidate_set", text)
