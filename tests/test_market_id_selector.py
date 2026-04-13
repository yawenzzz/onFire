import unittest

from polymarket_arb.venue.market_id_selector import select_market_ids_from_events


class MarketIdSelectorTests(unittest.TestCase):
    def test_extracts_market_slugs_from_events_payload(self) -> None:
        payload = {
            'events': [
                {'markets': [{'slug': 'm1'}, {'slug': 'm2'}]},
                {'markets': [{'slug': 'm3'}]},
            ]
        }
        ids = select_market_ids_from_events(payload, limit=2)
        self.assertEqual(ids, ['m1', 'm2'])

    def test_skips_markets_without_slug(self) -> None:
        payload = {'events': [{'markets': [{'id': 'x'}, {'slug': 'm2'}]}]}
        ids = select_market_ids_from_events(payload, limit=5)
        self.assertEqual(ids, ['m2'])
