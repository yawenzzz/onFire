from __future__ import annotations

from dataclasses import dataclass


@dataclass
class WebSocketClient:
    freshness_budget_ms: int = 1000

    def is_fresh(self, last_message_age_ms: int) -> bool:
        return last_message_age_ms <= self.freshness_budget_ms
