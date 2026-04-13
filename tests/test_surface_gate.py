import unittest

from polymarket_arb.app.bootstrap import startup_posture
from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.venue.surface_gate import SurfaceGate


class SurfaceGateTests(unittest.TestCase):
    def test_unresolved_surface_blocks_startup(self) -> None:
        posture = startup_posture(SurfaceGate(), LaunchGate())
        self.assertEqual(posture, "NO_TRADE")

    def test_ineligible_jurisdiction_blocks_startup(self) -> None:
        posture = startup_posture(
            SurfaceGate(surface_id="polymarket-us", jurisdiction_eligible=False),
            LaunchGate(surface_resolved=True, surface_id="polymarket-us"),
        )
        self.assertEqual(posture, "NO_TRADE")
