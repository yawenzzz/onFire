import unittest

from polymarket_arb.venue.subscription_planner import chunk_market_ids


class SubscriptionPlannerTests(unittest.TestCase):
    def test_chunks_market_ids_by_limit(self) -> None:
        groups = chunk_market_ids(['m1', 'm2', 'm3', 'm4', 'm5'], chunk_size=2)
        self.assertEqual(groups, [['m1', 'm2'], ['m3', 'm4'], ['m5']])

    def test_empty_input_returns_empty_groups(self) -> None:
        self.assertEqual(chunk_market_ids([], chunk_size=3), [])
