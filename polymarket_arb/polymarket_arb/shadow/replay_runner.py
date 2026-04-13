from __future__ import annotations

from polymarket_arb.models.types import CandidateBasket
from polymarket_arb.shadow.metrics import ShadowMetrics
from polymarket_arb.shadow.simulator import ShadowSimulator


class ReplayRunner:
    def run(self, session_id: str, surface_id: str, baskets: list[CandidateBasket]) -> ShadowMetrics:
        return ShadowSimulator().run(session_id=session_id, surface_id=surface_id, baskets=baskets)
