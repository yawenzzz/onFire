import unittest

from polymarket_arb.monitoring.gamma_client import GammaMarketClient


class StubGammaMarketClient(GammaMarketClient):
    def __init__(self, payload):
        super().__init__()
        self.payload = payload
        self.urls = []

    def _open_json(self, url: str):
        self.urls.append(url)
        return self.payload


class GammaMarketClientTests(unittest.TestCase):
    def test_fetch_events_uses_gamma_events_endpoint(self) -> None:
        client = StubGammaMarketClient([{"slug": "e1"}])

        data = client.fetch_events(limit=5, closed=False, offset=10)

        self.assertEqual(data[0]["slug"], "e1")
        self.assertIn("gamma-api.polymarket.com/events", client.urls[0])
        self.assertIn("limit=5", client.urls[0])
        self.assertIn("closed=false", client.urls[0])
        self.assertIn("offset=10", client.urls[0])
