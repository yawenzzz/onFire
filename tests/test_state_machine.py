import unittest

from polymarket_arb.execution.state_machine import ExecutionEngine, RouteResult
from polymarket_arb.models.types import CandidateBasket, CandidateState, Leg, MarketState


class StubRouter:
    def __init__(self, result: RouteResult, env_ok: bool = True) -> None:
        self._result = result
        self._env_ok = env_ok

    def env_ready(self) -> bool:
        return self._env_ok

    def place_basket(self, basket: CandidateBasket) -> RouteResult:
        return self._result


class StubHedger:
    def __init__(self, can_complete: bool) -> None:
        self._can_complete = can_complete
        self.completed = False
        self.flattened = False

    def can_complete_within_window(self, basket: CandidateBasket, timeout_ms: int) -> bool:
        return self._can_complete

    def complete(self, basket: CandidateBasket) -> None:
        self.completed = True

    def abort_and_flatten(self, basket: CandidateBasket) -> None:
        self.flattened = True


class StubReconciler:
    def __init__(self, should_match: bool = True) -> None:
        self.synced = False
        self.should_match = should_match

    def sync(self, basket: CandidateBasket, filled_market_ids=None):
        self.synced = True

    def matched(self) -> bool:
        return self.should_match


class StubKillSwitch:
    def __init__(self) -> None:
        self.reasons = []
        self.payloads = []

    def fire(self, reason: str, scope: str, payload=None) -> None:
        self.reasons.append((reason, scope))
        self.payloads.append(payload)


class ExecutionEngineTests(unittest.TestCase):
    def _basket(self) -> CandidateBasket:
        leg = Leg(
            market_id="m1",
            side="BUY",
            price=0.5,
            market_state=MarketState.OPEN,
            tick_valid=True,
            visible_depth_qty=10,
            preview_ok=True,
            clarification_hash="abc",
        )
        return CandidateBasket(
            group_id="g1",
            template_type="exhaustive_set",
            surface_id="polymarket-us",
            rule_hash_unchanged=True,
            clarification_hash_unchanged=True,
            market_state_all_open=True,
            preview_all_legs=True,
            zero_rebate_positive=True,
            pi_min_stress_usd=1.0,
            hedge_completion_prob=0.99,
            capital_efficiency=0.5,
            legs=[leg],
        )

    def test_full_fill_reaches_reconciling(self) -> None:
        reconciler = StubReconciler()
        engine = ExecutionEngine(
            router=StubRouter(RouteResult(full_fill=True, partial_fill=False)),
            hedger=StubHedger(can_complete=True),
            reconciler=reconciler,
            kill_switches=StubKillSwitch(),
        )
        basket = engine.run_candidate(self._basket())
        self.assertEqual(basket.state, CandidateState.RECONCILING)
        self.assertTrue(reconciler.synced)

    def test_partial_fill_timeout_triggers_abort_flatten(self) -> None:
        reconciler = StubReconciler()
        hedger = StubHedger(can_complete=False)
        kill_switch = StubKillSwitch()
        engine = ExecutionEngine(
            router=StubRouter(RouteResult(full_fill=False, partial_fill=True)),
            hedger=hedger,
            reconciler=reconciler,
            kill_switches=kill_switch,
        )
        basket = engine.run_candidate(self._basket())
        self.assertEqual(basket.state, CandidateState.RECONCILING)
        self.assertTrue(hedger.flattened)
        self.assertIn(("KILL_HEDGE_TIMEOUT", "g1"), kill_switch.reasons)
        self.assertEqual(kill_switch.payloads[0]["reason"], "KILL_HEDGE_TIMEOUT")

    def test_reconciliation_mismatch_kills_session(self) -> None:
        reconciler = StubReconciler(should_match=False)
        kill_switch = StubKillSwitch()
        engine = ExecutionEngine(
            router=StubRouter(RouteResult(full_fill=True, partial_fill=False)),
            hedger=StubHedger(can_complete=True),
            reconciler=reconciler,
            kill_switches=kill_switch,
        )
        basket = engine.run_candidate(self._basket())
        self.assertEqual(basket.state, CandidateState.KILLED)
        self.assertIn(("KILL_RECONCILIATION_MISMATCH", "g1"), kill_switch.reasons)
        self.assertEqual(kill_switch.payloads[0]["reason"], "KILL_RECONCILIATION_MISMATCH")
