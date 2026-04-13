import unittest

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.config.validator import validate_launch_gate


class ConfigValidatorTests(unittest.TestCase):
    def test_rejects_inconsistent_surface_configuration(self) -> None:
        problems = validate_launch_gate(LaunchGate(surface_resolved=False, surface_id="polymarket-us"))
        self.assertIn("surface_resolved is false but surface_id is set", problems)

    def test_accepts_consistent_fail_closed_default(self) -> None:
        problems = validate_launch_gate(LaunchGate())
        self.assertEqual(problems, [])
