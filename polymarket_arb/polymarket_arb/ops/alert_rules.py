from __future__ import annotations

from polymarket_arb.ops.health_model import HealthStatus


def evaluate_alert_rules(status: HealthStatus) -> list[str]:
    alerts: list[str] = []
    if not status.feed_fresh:
        alerts.append('feed_stale')
    if not status.archive_ok:
        alerts.append('archive_failure')
    if not status.parse_error_rate_ok:
        alerts.append('parse_error_rate')
    return alerts
