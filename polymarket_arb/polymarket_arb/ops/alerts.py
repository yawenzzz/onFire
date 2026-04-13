from __future__ import annotations

from polymarket_arb.shadow.metrics import ShadowMetrics


def build_redline_alerts(metrics: ShadowMetrics) -> list[str]:
    alerts: list[str] = []
    if metrics.preview_success_rate < 1.0:
        alerts.append("preview_success_rate < 1.0")
    if metrics.api_429_count > 0:
        alerts.append("api_429_count > 0")
    if metrics.reconciliation_mismatch_count > 0:
        alerts.append("reconciliation_mismatch_count > 0")
    if metrics.false_positive_rate > 0.05:
        alerts.append("false_positive_rate > 0.05")
    return alerts
