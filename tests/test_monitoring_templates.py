import json
import unittest
from pathlib import Path


class MonitoringTemplatesTests(unittest.TestCase):
    def test_prometheus_template_exists(self) -> None:
        text = Path('monitoring/prometheus.yml').read_text()
        self.assertIn('/status', text)

    def test_alertmanager_template_exists(self) -> None:
        text = Path('monitoring/alertmanager.yml').read_text()
        self.assertIn('route:', text)

    def test_grafana_dashboard_template_exists(self) -> None:
        data = json.loads(Path('monitoring/grafana-dashboard.json').read_text())
        self.assertIn('title', data)
