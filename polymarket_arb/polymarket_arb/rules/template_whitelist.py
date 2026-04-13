from __future__ import annotations

from dataclasses import dataclass

SAFE_TEMPLATES = {
    "exhaustive_set",
    "directional_ladder",
    "offset_structure",
}


@dataclass(frozen=True)
class TemplateMatch:
    template_type: str
    allowed: bool


def is_whitelisted(template_type: str) -> bool:
    return template_type in SAFE_TEMPLATES


def match_template(
    outcome_count: int,
    ordered_thresholds: bool,
    offset_relation: bool,
) -> TemplateMatch:
    if ordered_thresholds and outcome_count >= 2:
        return TemplateMatch(template_type="directional_ladder", allowed=True)
    if offset_relation:
        return TemplateMatch(template_type="offset_structure", allowed=True)
    if outcome_count >= 2:
        return TemplateMatch(template_type="exhaustive_set", allowed=True)
    return TemplateMatch(template_type="unsupported", allowed=False)
