from __future__ import annotations

from dataclasses import dataclass


@dataclass
class ShadowMetrics:
    session_id: str
    surface_id: str
    preview_success_rate: float
    hedge_completion_rate_shadow: float
    false_positive_rate: float
    api_429_count: int
    reconciliation_mismatch_count: int

    @classmethod
    def from_counts(
        cls,
        session_id: str,
        surface_id: str,
        preview_successes: int,
        preview_attempts: int,
        hedge_completions: int,
        hedge_attempts: int,
        false_positives: int,
        candidate_count: int,
        api_429_count: int,
        reconciliation_mismatch_count: int,
    ) -> "ShadowMetrics":
        preview_success_rate = (
            preview_successes / preview_attempts if preview_attempts else 0.0
        )
        hedge_completion_rate_shadow = (
            hedge_completions / hedge_attempts if hedge_attempts else 0.0
        )
        false_positive_rate = (
            false_positives / candidate_count if candidate_count else 0.0
        )
        return cls(
            session_id=session_id,
            surface_id=surface_id,
            preview_success_rate=preview_success_rate,
            hedge_completion_rate_shadow=hedge_completion_rate_shadow,
            false_positive_rate=false_positive_rate,
            api_429_count=api_429_count,
            reconciliation_mismatch_count=reconciliation_mismatch_count,
        )
