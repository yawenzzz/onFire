import unittest

from polymarket_arb.execution.kill_switch_event import KillSwitchEvent
from polymarket_arb.ops.incident_writer import IncidentWriter


class IncidentWriterTests(unittest.TestCase):
    def test_records_payloads(self) -> None:
        writer = IncidentWriter()
        event = KillSwitchEvent(
            reason="KILL_HEDGE_TIMEOUT",
            scope="g1",
            trigger_metric="hedge_window_ms",
            observed_value=4100,
            threshold=3000,
            action="abort_flatten",
        )
        writer.write(event)
        self.assertEqual(len(writer.events), 1)
        self.assertEqual(writer.events[0]["reason"], "KILL_HEDGE_TIMEOUT")
