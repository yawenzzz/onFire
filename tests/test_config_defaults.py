import unittest

from polymarket_arb.config.defaults import default_launch_gate


class ConfigDefaultsTests(unittest.TestCase):
    def test_default_launch_gate_is_fail_closed(self) -> None:
        gate = default_launch_gate()
        self.assertFalse(gate.launch_eligible())
        self.assertEqual(gate.posture(), "NO_TRADE")
