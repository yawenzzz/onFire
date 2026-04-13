import tempfile
import time
import unittest
import urllib.request
import json
from pathlib import Path

from polymarket_arb.service.status_service import start_status_service


class StatusServiceTests(unittest.TestCase):
    def test_start_status_service_serves_http(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'metrics.json').write_text(json.dumps({'preview_success_rate': 1.0}))
            (root / 'health.json').write_text(json.dumps({'healthy': True}))
            (root / 'alerts.json').write_text(json.dumps({'alerts': []}))
            server = start_status_service(root, host='127.0.0.1', port=0)
            try:
                port = server.server_address[1]
                time.sleep(0.05)
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/status', timeout=5) as r:
                    payload = json.loads(r.read().decode('utf-8'))
                self.assertTrue(payload['health']['healthy'])
            finally:
                server.shutdown()
                server.server_close()
