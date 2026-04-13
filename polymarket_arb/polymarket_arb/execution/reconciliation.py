from __future__ import annotations

from dataclasses import dataclass, field

from polymarket_arb.models.types import CandidateBasket


@dataclass
class ReconciliationResult:
    matched: bool
    missing_market_ids: list[str] = field(default_factory=list)
    unexpected_market_ids: list[str] = field(default_factory=list)


class Reconciler:
    def __init__(self) -> None:
        self._last_result = ReconciliationResult(matched=True)

    def sync(
        self,
        basket: CandidateBasket,
        filled_market_ids: list[str] | None = None,
    ) -> ReconciliationResult:
        expected = [leg.market_id for leg in basket.legs]
        actual = filled_market_ids if filled_market_ids is not None else expected
        missing = [market_id for market_id in expected if market_id not in actual]
        unexpected = [market_id for market_id in actual if market_id not in expected]
        self._last_result = ReconciliationResult(
            matched=not missing and not unexpected,
            missing_market_ids=missing,
            unexpected_market_ids=unexpected,
        )
        return self._last_result

    def matched(self) -> bool:
        return self._last_result.matched
