import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.ops.metrics_http import render_metrics_response


class MetricsHttpTests(unittest.TestCase):
    def test_renders_json_metrics_response(self) -> None:
        body = render_metrics_response({'preview_success_rate': 1.0, 'reconnect_count': 2})
        data = json.loads(body)
        self.assertEqual(data['preview_success_rate'], 1.0)
        self.assertEqual(data['reconnect_count'], 2)
