import unittest

from polymarket_arb.rules.drift_guard import evaluate_hash_guard


class DriftGuardTests(unittest.TestCase):
    def test_returns_ok_when_rule_and_clarification_match(self) -> None:
        decision = evaluate_hash_guard(rule_ok=True, clarification_ok=True)
        self.assertTrue(decision.allowed)
        self.assertEqual(decision.reasons, [])

    def test_blocks_when_rule_hash_drifted(self) -> None:
        decision = evaluate_hash_guard(rule_ok=False, clarification_ok=True)
        self.assertFalse(decision.allowed)
        self.assertIn("rule hash drift", decision.reasons)

    def test_blocks_when_clarification_hash_drifted(self) -> None:
        decision = evaluate_hash_guard(rule_ok=True, clarification_ok=False)
        self.assertFalse(decision.allowed)
        self.assertIn("clarification hash drift", decision.reasons)
