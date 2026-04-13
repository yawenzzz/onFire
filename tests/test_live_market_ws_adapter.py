import json
import unittest

from polymarket_arb.venue.live_market_ws_adapter import LiveMarketWSAdapter


class LiveMarketWSAdapterTests(unittest.TestCase):
    def test_parses_json_message_to_snapshot(self) -> None:
        adapter = LiveMarketWSAdapter()
        raw = json.dumps({
            'market_id': 'm1',
            'market_state': 'OPEN',
            'best_bid': 0.4,
            'best_ask': 0.5,
        })
        snapshot = adapter.parse_message(raw)
        self.assertEqual(snapshot.market_id, 'm1')
        self.assertEqual(snapshot.best_ask, 0.5)

    def test_ignores_non_dict_json(self) -> None:
        adapter = LiveMarketWSAdapter()
        self.assertIsNone(adapter.parse_message('[1,2,3]'))
