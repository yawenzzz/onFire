import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.daemon_heartbeat import write_heartbeat


class DaemonHeartbeatTests(unittest.TestCase):
    def test_writes_heartbeat_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'heartbeat.json'
            write_heartbeat(path, service='capture', alive=True)
            data = json.loads(path.read_text())
            self.assertEqual(data['service'], 'capture')
            self.assertTrue(data['alive'])
