import unittest

from polymarket_arb.venue.public_gateway_client import PublicGatewayClient


class StubGatewayClient(PublicGatewayClient):
    def __init__(self, payload):
        super().__init__()
        self.payload = payload
        self.urls = []

    def _open_json(self, url: str):
        self.urls.append(url)
        return self.payload


class PublicGatewayClientTests(unittest.TestCase):
    def test_fetch_events_uses_events_endpoint(self) -> None:
        client = StubGatewayClient({'events': [{'slug': 'e1'}]})
        data = client.fetch_events(limit=1)
        self.assertEqual(data['events'][0]['slug'], 'e1')
        self.assertIn('/v1/events?limit=1', client.urls[0])

    def test_fetch_market_book_uses_market_slug(self) -> None:
        client = StubGatewayClient({'marketData': {'marketSlug': 'm1'}})
        data = client.fetch_market_book('m1')
        self.assertEqual(data['marketData']['marketSlug'], 'm1')
        self.assertIn('/v1/markets/m1/book', client.urls[0])
