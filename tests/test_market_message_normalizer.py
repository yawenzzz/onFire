import unittest

from polymarket_arb.data.market_message_normalizer import normalize_market_message


class MarketMessageNormalizerTests(unittest.TestCase):
    def test_normalizes_minimal_message_to_snapshot(self) -> None:
        snapshot = normalize_market_message({
            'market_id': 'm1',
            'market_state': 'OPEN',
            'best_bid': 0.41,
            'best_ask': 0.44,
        })
        self.assertEqual(snapshot.market_id, 'm1')
        self.assertEqual(snapshot.market_state, 'OPEN')
        self.assertEqual(snapshot.best_bid, 0.41)
        self.assertEqual(snapshot.best_ask, 0.44)

    def test_raises_on_missing_required_fields(self) -> None:
        with self.assertRaises(ValueError):
            normalize_market_message({'market_id': 'm1'})
