import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.dashboard_bundle import write_dashboard_bundle


class DashboardBundleTests(unittest.TestCase):
    def test_writes_dashboard_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = write_dashboard_bundle(root, {'preview_success_rate': 1.0})
            self.assertTrue(path.exists())
            self.assertEqual(json.loads(path.read_text())['preview_success_rate'], 1.0)
