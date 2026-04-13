from __future__ import annotations

from dataclasses import dataclass


@dataclass
class SurfaceGate:
    surface_id: str = "unresolved"
    jurisdiction_eligible: bool = False

    def resolved(self) -> bool:
        return self.surface_id != "unresolved"

    def eligible(self) -> bool:
        return self.jurisdiction_eligible

    def fail_reason(self) -> str | None:
        if not self.resolved():
            return "venue/product surface unresolved"
        if not self.eligible():
            return "geographic/compliance eligibility unresolved"
        return None
