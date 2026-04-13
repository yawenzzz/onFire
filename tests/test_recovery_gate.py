import unittest

from polymarket_arb.ops.recovery_gate import RecoveryGate


class RecoveryGateTests(unittest.TestCase):
    def test_blocks_restart_when_open_critical_incident_exists(self) -> None:
        gate = RecoveryGate()
        self.assertFalse(gate.allow_restart(has_open_critical_incident=True, reconciliation_clean=True))

    def test_allows_restart_only_when_clean(self) -> None:
        gate = RecoveryGate()
        self.assertTrue(gate.allow_restart(has_open_critical_incident=False, reconciliation_clean=True))
