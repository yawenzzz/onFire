import unittest
from pathlib import Path


class MonitoringStackDocsTests(unittest.TestCase):
    def test_monitoring_compose_exists(self) -> None:
        text = Path('monitoring/docker-compose.monitoring.yml').read_text()
        self.assertIn('prometheus', text)
        self.assertIn('grafana', text)
        self.assertIn('alertmanager', text)

    def test_alert_rules_file_exists(self) -> None:
        text = Path('monitoring/alerts.yml').read_text()
        self.assertIn('groups:', text)
        self.assertIn('feed_stale', text)

    def test_monitoring_setup_doc_exists(self) -> None:
        text = Path('docs/monitoring-setup.md').read_text()
        self.assertIn('Prometheus', text)
        self.assertIn('Grafana', text)
        self.assertIn('Alertmanager', text)
