from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


class MarketState(str, Enum):
    OPEN = "OPEN"
    PREOPEN = "PREOPEN"
    SUSPENDED = "SUSPENDED"
    HALTED = "HALTED"
    EXPIRED = "EXPIRED"
    TERMINATED = "TERMINATED"
    MATCH_AND_CLOSE_AUCTION = "MATCH_AND_CLOSE_AUCTION"


class CandidateState(str, Enum):
    DISCOVERED = "DISCOVERED"
    STRUCTURE_VALIDATED = "STRUCTURE_VALIDATED"
    PRICED = "PRICED"
    PREVIEW_READY = "PREVIEW_READY"
    ORDERING = "ORDERING"
    PARTIAL_FILL = "PARTIAL_FILL"
    HEDGED = "HEDGED"
    ABORT_FLATTEN = "ABORT_FLATTEN"
    RECONCILING = "RECONCILING"
    NO_TRADE = "NO_TRADE"
    KILLED = "KILLED"


@dataclass
class Leg:
    market_id: str
    side: str
    price: float
    market_state: MarketState
    tick_valid: bool
    visible_depth_qty: float
    preview_ok: bool
    clarification_hash: str

    def is_tradeable(self) -> bool:
        return (
            self.market_state is MarketState.OPEN
            and self.tick_valid
            and self.visible_depth_qty > 0
            and self.preview_ok
            and 0.01 <= self.price <= 0.99
        )


@dataclass
class CandidateBasket:
    group_id: str
    template_type: str
    surface_id: str
    rule_hash_unchanged: bool
    clarification_hash_unchanged: bool
    market_state_all_open: bool
    preview_all_legs: bool
    zero_rebate_positive: bool
    pi_min_stress_usd: float
    hedge_completion_prob: float
    capital_efficiency: float
    ambiguity_penalty: float = 0.0
    ops_penalty: float = 0.0
    state: CandidateState = CandidateState.DISCOVERED
    legs: list[Leg] = field(default_factory=list)

    def is_structurally_safe(self) -> bool:
        return (
            bool(self.legs)
            and self.rule_hash_unchanged
            and self.clarification_hash_unchanged
            and self.market_state_all_open
            and self.preview_all_legs
            and all(leg.is_tradeable() for leg in self.legs)
        )
