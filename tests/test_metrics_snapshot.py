import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.metrics_snapshot import write_metrics_snapshot


class MetricsSnapshotTests(unittest.TestCase):
    def test_writes_metrics_snapshot_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'metrics.json'
            write_metrics_snapshot(path, {'preview_success_rate': 1.0})
            data = json.loads(path.read_text())
            self.assertEqual(data['preview_success_rate'], 1.0)
