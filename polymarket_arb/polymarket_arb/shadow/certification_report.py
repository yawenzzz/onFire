from __future__ import annotations

from polymarket_arb.shadow.metrics import ShadowMetrics


def build_certification_report(verdict: str, metrics: ShadowMetrics) -> dict:
    return {
        "verdict": verdict,
        "session_id": metrics.session_id,
        "surface_id": metrics.surface_id,
        "preview_success_rate": metrics.preview_success_rate,
        "hedge_completion_rate_shadow": metrics.hedge_completion_rate_shadow,
        "false_positive_rate": metrics.false_positive_rate,
        "api_429_count": metrics.api_429_count,
        "reconciliation_mismatch_count": metrics.reconciliation_mismatch_count,
    }
