import unittest

from polymarket_arb.venue.websocket_client import WebSocketClient


class WebSocketClientTests(unittest.TestCase):
    def test_feed_is_stale_when_budget_exceeded(self) -> None:
        client = WebSocketClient(freshness_budget_ms=1000)
        self.assertFalse(client.is_fresh(last_message_age_ms=1500))

    def test_feed_is_fresh_within_budget(self) -> None:
        client = WebSocketClient(freshness_budget_ms=1000)
        self.assertTrue(client.is_fresh(last_message_age_ms=250))
