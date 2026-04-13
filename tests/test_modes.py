import unittest

from polymarket_arb.app.modes import choose_mode


class ModesTests(unittest.TestCase):
    def test_choose_research_when_surface_unresolved(self) -> None:
        self.assertEqual(choose_mode(surface_resolved=False, certified=False), "research")

    def test_choose_shadow_when_surface_resolved_but_not_certified(self) -> None:
        self.assertEqual(choose_mode(surface_resolved=True, certified=False), "shadow")

    def test_choose_live_capable_when_certified(self) -> None:
        self.assertEqual(choose_mode(surface_resolved=True, certified=True), "live_capable")
