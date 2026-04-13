from __future__ import annotations

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.models.types import CandidateBasket


class PreTradeGate:
    def allow(self, basket: CandidateBasket, gate: LaunchGate) -> bool:
        return (
            gate.surface_resolved
            and gate.jurisdiction_eligible
            and basket.market_state_all_open
            and basket.preview_all_legs
            and basket.zero_rebate_positive
            and basket.is_structurally_safe()
        )
