import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.dashboard_refresh import refresh_dashboard_bundle


class DashboardRefreshTests(unittest.TestCase):
    def test_refresh_dashboard_bundle_writes_dashboard(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = refresh_dashboard_bundle(root, {'preview_success_rate': 1.0})
            self.assertTrue(path.exists())
            self.assertIn('dashboard.json', str(path))
