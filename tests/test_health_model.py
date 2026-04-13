import unittest

from polymarket_arb.ops.health_model import HealthStatus


class HealthModelTests(unittest.TestCase):
    def test_unhealthy_when_feed_stale(self) -> None:
        status = HealthStatus(feed_fresh=False, archive_ok=True, parse_error_rate_ok=True)
        self.assertFalse(status.healthy())

    def test_healthy_when_all_components_green(self) -> None:
        status = HealthStatus(feed_fresh=True, archive_ok=True, parse_error_rate_ok=True)
        self.assertTrue(status.healthy())
