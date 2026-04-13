import unittest

from polymarket_arb.venue.compliance_gate import ComplianceGate


class ComplianceGateTests(unittest.TestCase):
    def test_unconfirmed_eligibility_fails_closed(self) -> None:
        gate = ComplianceGate(surface_id="polymarket-us")
        self.assertFalse(gate.eligible())
        self.assertEqual(gate.fail_reason(), "geographic/compliance eligibility unresolved")

    def test_confirmed_eligibility_clears_gate(self) -> None:
        gate = ComplianceGate(surface_id="polymarket-us", jurisdiction_eligible=True)
        self.assertTrue(gate.eligible())
        self.assertIsNone(gate.fail_reason())
