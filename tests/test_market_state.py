import unittest

from polymarket_arb.venue.market_state import is_tradeable_market_state


class MarketStateHelpersTests(unittest.TestCase):
    def test_open_is_tradeable(self) -> None:
        self.assertTrue(is_tradeable_market_state("OPEN"))

    def test_non_open_is_not_tradeable(self) -> None:
        self.assertFalse(is_tradeable_market_state("HALTED"))
        self.assertFalse(is_tradeable_market_state("PREOPEN"))
