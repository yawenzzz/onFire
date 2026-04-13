from __future__ import annotations

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.shadow.metrics import ShadowMetrics


class ShadowCertification:
    def evaluate(self, gate: LaunchGate, metrics: ShadowMetrics | None = None) -> str:
        if not gate.surface_resolved or not gate.jurisdiction_eligible:
            return "CERTIFICATION_BLOCKED"

        if metrics is None:
            return "CERTIFICATION_INCOMPLETE"

        if (
            gate.launch_eligible()
            and metrics.preview_success_rate == 1.0
            and metrics.hedge_completion_rate_shadow >= 0.99
            and metrics.false_positive_rate <= 0.05
            and metrics.api_429_count == 0
            and metrics.reconciliation_mismatch_count == 0
            and gate.shadow_window_days >= 14
        ):
            return "LIVE_CAPABLE_READY"

        return "CERTIFICATION_INCOMPLETE"
