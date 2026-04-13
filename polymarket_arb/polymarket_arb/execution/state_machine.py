from __future__ import annotations

from dataclasses import dataclass

from polymarket_arb.execution.kill_switch_event import KillSwitchEvent
from polymarket_arb.models.types import CandidateBasket, CandidateState
from polymarket_arb.strategy.scorer import RobustScorer


@dataclass
class RouteResult:
    full_fill: bool
    partial_fill: bool


class ExecutionEngine:
    def __init__(self, router, hedger, reconciler, kill_switches) -> None:
        self.router = router
        self.hedger = hedger
        self.reconciler = reconciler
        self.kill_switches = kill_switches

    def run_candidate(self, basket: CandidateBasket) -> CandidateBasket:
        basket.state = CandidateState.STRUCTURE_VALIDATED
        score = RobustScorer().score(basket)
        if score.rejected or not self.router.env_ready():
            basket.state = CandidateState.NO_TRADE
            return basket

        basket.state = CandidateState.PRICED
        basket.state = CandidateState.PREVIEW_READY
        return self._order_and_hedge(basket)

    def _emit_kill(self, reason: str, basket: CandidateBasket, trigger_metric: str, observed_value: float, threshold: float, action: str) -> None:
        event = KillSwitchEvent(
            reason=reason,
            scope=basket.group_id,
            trigger_metric=trigger_metric,
            observed_value=observed_value,
            threshold=threshold,
            action=action,
        )
        self.kill_switches.fire(reason, basket.group_id, payload=event.to_payload())

    def _reconcile(self, basket: CandidateBasket) -> CandidateBasket:
        self.reconciler.sync(basket)
        if hasattr(self.reconciler, "matched") and not self.reconciler.matched():
            self._emit_kill(
                "KILL_RECONCILIATION_MISMATCH",
                basket,
                trigger_metric="reconciliation_match",
                observed_value=0,
                threshold=1,
                action="kill_session",
            )
            basket.state = CandidateState.KILLED
            return basket
        basket.state = CandidateState.RECONCILING
        return basket

    def _order_and_hedge(self, basket: CandidateBasket) -> CandidateBasket:
        basket.state = CandidateState.ORDERING
        result = self.router.place_basket(basket)

        if result.full_fill:
            basket.state = CandidateState.HEDGED
            return self._reconcile(basket)

        if result.partial_fill:
            basket.state = CandidateState.PARTIAL_FILL
            if self.hedger.can_complete_within_window(basket, timeout_ms=3000):
                self.hedger.complete(basket)
                basket.state = CandidateState.HEDGED
            else:
                self._emit_kill(
                    "KILL_HEDGE_TIMEOUT",
                    basket,
                    trigger_metric="hedge_window_ms",
                    observed_value=3001,
                    threshold=3000,
                    action="abort_flatten",
                )
                basket.state = CandidateState.ABORT_FLATTEN
                self.hedger.abort_and_flatten(basket)
            return self._reconcile(basket)

        basket.state = CandidateState.NO_TRADE
        return basket
