from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class DriftDecision:
    allowed: bool
    reasons: list[str] = field(default_factory=list)


def evaluate_hash_guard(rule_ok: bool, clarification_ok: bool) -> DriftDecision:
    reasons: list[str] = []
    if not rule_ok:
        reasons.append("rule hash drift")
    if not clarification_ok:
        reasons.append("clarification hash drift")
    return DriftDecision(allowed=not reasons, reasons=reasons)
