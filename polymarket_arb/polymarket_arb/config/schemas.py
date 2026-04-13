from __future__ import annotations

from dataclasses import dataclass


@dataclass
class LaunchGate:
    surface_resolved: bool = False
    surface_id: str = "unresolved"
    jurisdiction_eligible: bool = False
    market_state_all_open: bool = False
    preview_success_rate: float = 0.0
    invalid_tick_or_price_reject_rate: float = 1.0
    api_429_count: int = 1
    ambiguous_rule_trade_count: int = 1
    collateral_return_dependency_for_safety: int = 1
    hedge_completion_rate_shadow: float = 0.0
    false_positive_rate: float = 1.0
    shadow_window_days: int = 0

    def launch_eligible(self) -> bool:
        return (
            self.surface_resolved
            and self.jurisdiction_eligible
            and self.market_state_all_open
            and self.preview_success_rate == 1.0
            and self.invalid_tick_or_price_reject_rate == 0.0
            and self.api_429_count == 0
            and self.ambiguous_rule_trade_count == 0
            and self.collateral_return_dependency_for_safety == 0
        )

    def posture(self) -> str:
        return "LIVE_CAPABLE_READY" if self.launch_eligible() else "NO_TRADE"
