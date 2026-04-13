import unittest

from polymarket_arb.venue.multi_market_scheduler import build_subscription_batches


class MultiMarketSchedulerTests(unittest.TestCase):
    def test_builds_batched_subscription_messages(self) -> None:
        batches = build_subscription_batches(['m1', 'm2', 'm3'], chunk_size=2)
        self.assertEqual(len(batches), 2)
        self.assertEqual(batches[0]['market_ids'], ['m1', 'm2'])
        self.assertEqual(batches[1]['market_ids'], ['m3'])
