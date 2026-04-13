from __future__ import annotations

from polymarket_arb.models.types import CandidateBasket
from polymarket_arb.shadow.metrics import ShadowMetrics


class ShadowSimulator:
    def run(
        self,
        session_id: str,
        surface_id: str,
        baskets: list[CandidateBasket],
    ) -> ShadowMetrics:
        candidate_count = len(baskets)
        preview_successes = sum(1 for basket in baskets if basket.preview_all_legs)
        hedge_completions = sum(1 for basket in baskets if basket.hedge_completion_prob >= 0.99)
        false_positives = sum(1 for basket in baskets if basket.pi_min_stress_usd <= 0)
        return ShadowMetrics.from_counts(
            session_id=session_id,
            surface_id=surface_id,
            preview_successes=preview_successes,
            preview_attempts=candidate_count,
            hedge_completions=hedge_completions,
            hedge_attempts=candidate_count,
            false_positives=false_positives,
            candidate_count=candidate_count,
            api_429_count=0,
            reconciliation_mismatch_count=0,
        )
