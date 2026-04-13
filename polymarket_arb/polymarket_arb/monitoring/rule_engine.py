from __future__ import annotations

from polymarket_arb.monitoring.models import RuleEngineResult
from polymarket_arb.monitoring.rules import classify_complete_bucket_event


def evaluate_complete_bucket_event(event: dict) -> RuleEngineResult:
    classification = classify_complete_bucket_event(event)
    if classification.is_complete:
        template_type = classification.structure_type
        if template_type in {"market_cap_buckets", "complete_bucket"}:
            template_type = "ipo_market_cap_complete_bucket"
        if template_type in {"count_buckets", "count_bucket"}:
            template_type = "count_complete_bucket"
        return RuleEngineResult(
            is_candidate=True,
            template_type=template_type,
            rejection_reason=None,
            gross_edge=classification.gross_edge,
        )

    rejection_reason = classification.rejection_reason or "unknown"
    return RuleEngineResult(
        is_candidate=False,
        template_type=None,
        rejection_reason=rejection_reason,
        gross_edge=classification.gross_edge,
    )
