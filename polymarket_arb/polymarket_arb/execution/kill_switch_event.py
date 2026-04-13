from __future__ import annotations

from dataclasses import dataclass


@dataclass
class KillSwitchEvent:
    reason: str
    scope: str
    trigger_metric: str
    observed_value: float
    threshold: float
    action: str

    def to_payload(self) -> dict:
        return {
            "reason": self.reason,
            "scope": self.scope,
            "trigger_metric": self.trigger_metric,
            "observed_value": self.observed_value,
            "threshold": self.threshold,
            "action": self.action,
        }
