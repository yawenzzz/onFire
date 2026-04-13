from __future__ import annotations

from polymarket_arb.rules.template_whitelist import TemplateMatch, match_template


def parse_structure(
    outcome_count: int,
    ordered_thresholds: bool,
    offset_relation: bool,
) -> TemplateMatch:
    return match_template(
        outcome_count=outcome_count,
        ordered_thresholds=ordered_thresholds,
        offset_relation=offset_relation,
    )
