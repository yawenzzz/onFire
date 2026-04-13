import unittest

from polymarket_arb.execution.kill_switch_event import KillSwitchEvent


class KillSwitchEventTests(unittest.TestCase):
    def test_builds_payload_with_reason_scope_and_thresholds(self) -> None:
        event = KillSwitchEvent(
            reason="KILL_HEDGE_TIMEOUT",
            scope="g1",
            trigger_metric="hedge_window_ms",
            observed_value=4100,
            threshold=3000,
            action="abort_flatten",
        )
        payload = event.to_payload()
        self.assertEqual(payload["reason"], "KILL_HEDGE_TIMEOUT")
        self.assertEqual(payload["scope"], "g1")
        self.assertEqual(payload["threshold"], 3000)
