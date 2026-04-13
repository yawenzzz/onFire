import unittest

from polymarket_arb.venue.multi_market_manager import build_subscription_plan


class MultiMarketManagerTests(unittest.TestCase):
    def test_builds_subscription_messages_in_batches(self) -> None:
        plan = build_subscription_plan(['m1', 'm2', 'm3'], chunk_size=2)
        self.assertEqual(plan['market_count'], 3)
        self.assertEqual(len(plan['subscriptions']), 2)
        self.assertEqual(plan['subscriptions'][0]['market_ids'], ['m1', 'm2'])
        self.assertEqual(plan['subscriptions'][1]['market_ids'], ['m3'])

    def test_empty_ids_yield_empty_plan(self) -> None:
        plan = build_subscription_plan([], chunk_size=2)
        self.assertEqual(plan['market_count'], 0)
        self.assertEqual(plan['subscriptions'], [])
