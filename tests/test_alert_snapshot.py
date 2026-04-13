import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.alert_snapshot import write_alert_snapshot


class AlertSnapshotTests(unittest.TestCase):
    def test_writes_alert_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'alerts.json'
            write_alert_snapshot(path, ['feed_stale'])
            data = json.loads(path.read_text())
            self.assertEqual(data['alerts'], ['feed_stale'])
