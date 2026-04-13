import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.health_snapshot import write_health_snapshot
from polymarket_arb.ops.health_model import HealthStatus


class HealthSnapshotTests(unittest.TestCase):
    def test_writes_health_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'health.json'
            write_health_snapshot(path, HealthStatus(feed_fresh=True, archive_ok=True, parse_error_rate_ok=True))
            data = json.loads(path.read_text())
            self.assertTrue(data['healthy'])
