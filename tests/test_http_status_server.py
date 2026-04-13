import json
import tempfile
import time
import unittest
import urllib.request
from pathlib import Path

from polymarket_arb.ops.http_status_server import start_status_server


class HttpStatusServerTests(unittest.TestCase):
    def test_serves_status_payload(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'metrics.json').write_text(json.dumps({'preview_success_rate': 1.0}))
            (root / 'health.json').write_text(json.dumps({'healthy': True}))
            (root / 'alerts.json').write_text(json.dumps({'alerts': []}))
            server = start_status_server(root, host='127.0.0.1', port=0)
            try:
                port = server.server_address[1]
                time.sleep(0.05)
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/status', timeout=5) as r:
                    data = json.loads(r.read().decode('utf-8'))
                self.assertTrue(data['health']['healthy'])
                self.assertEqual(data['metrics']['preview_success_rate'], 1.0)
            finally:
                server.shutdown()
                server.server_close()
