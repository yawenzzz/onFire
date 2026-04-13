import unittest

from polymarket_arb.ops.alert_rules import evaluate_alert_rules
from polymarket_arb.ops.health_model import HealthStatus


class AlertRulesTests(unittest.TestCase):
    def test_generates_alerts_for_unhealthy_state(self) -> None:
        alerts = evaluate_alert_rules(HealthStatus(feed_fresh=False, archive_ok=False, parse_error_rate_ok=True))
        self.assertIn('feed_stale', alerts)
        self.assertIn('archive_failure', alerts)

    def test_no_alerts_when_healthy(self) -> None:
        alerts = evaluate_alert_rules(HealthStatus(feed_fresh=True, archive_ok=True, parse_error_rate_ok=True))
        self.assertEqual(alerts, [])
