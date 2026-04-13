from __future__ import annotations

from dataclasses import dataclass


@dataclass
class ComplianceGate:
    surface_id: str = "unresolved"
    jurisdiction_eligible: bool = False

    def eligible(self) -> bool:
        return self.surface_id != "unresolved" and self.jurisdiction_eligible

    def fail_reason(self) -> str | None:
        if self.surface_id == "unresolved":
            return "venue/product surface unresolved"
        if not self.jurisdiction_eligible:
            return "geographic/compliance eligibility unresolved"
        return None
