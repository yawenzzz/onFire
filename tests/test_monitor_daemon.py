import tempfile
import unittest
from pathlib import Path

from polymarket_arb.service.monitor_daemon import run_monitor_iteration


class StubScanner:
    def scan(self, limit: int = 50):
        from polymarket_arb.monitoring.models import EventScanResult, MonitorSnapshot

        candidate = EventScanResult(
            slug="fannie-mae-ipo-closing-market-cap",
            title="Fannie Mae IPO Closing Market Cap",
            template_type="complete_bucket",
            is_complete=True,
            open_market_count=7,
            total_market_count=7,
            sum_ask=0.999,
            gross_edge=0.001,
            cost_adjusted_edge=-0.029,
            rejection_reason=None,
            category="Business",
            questions=[],
        )
        rejected = EventScanResult(
            slug="consensys-ipo-closing-market-cap-above",
            title="Consensys IPO closing market cap above ___ ?",
            template_type=None,
            is_complete=False,
            open_market_count=3,
            total_market_count=3,
            sum_ask=0.70,
            gross_edge=0.30,
            cost_adjusted_edge=0.27,
            rejection_reason="overlapping_thresholds",
            category="Crypto",
            questions=[],
        )
        return MonitorSnapshot(
            iteration=1,
            event_count=20,
            complete_event_count=1,
            candidate_count=1,
            best_gross_edge=0.001,
            best_cost_adjusted_edge=-0.029,
            scan_duration_seconds=1.2,
            candidates=[candidate],
            rejections=[rejected],
            rejection_counts={"overlapping_thresholds": 1},
            data_source="gamma-api",
            status="ok",
        )


class MonitorDaemonTests(unittest.TestCase):
    def test_run_monitor_iteration_writes_runtime_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            snapshot = run_monitor_iteration(root=root, scanner=StubScanner(), limit=25)

            self.assertEqual(snapshot.event_count, 20)
            self.assertTrue((root / "metrics.json").exists())
            self.assertTrue((root / "health.json").exists())
            self.assertTrue((root / "alerts.json").exists())
            self.assertTrue((root / "dashboard.json").exists())
            dashboard = (root / "dashboard.json").read_text()
            self.assertIn("watched_events", dashboard)
            self.assertIn("category_counts", dashboard)
