import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.metrics_http_server import build_status_payload


class MetricsHttpServerTests(unittest.TestCase):
    def test_build_status_payload_reads_metrics_health_alerts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'metrics.json').write_text(json.dumps({'preview_success_rate': 1.0}))
            (root / 'health.json').write_text(json.dumps({'healthy': True}))
            (root / 'alerts.json').write_text(json.dumps({'alerts': []}))
            payload = build_status_payload(root)
            self.assertTrue(payload['health']['healthy'])
            self.assertEqual(payload['metrics']['preview_success_rate'], 1.0)
