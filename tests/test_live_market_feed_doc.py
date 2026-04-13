import unittest
from pathlib import Path


class LiveMarketFeedDocTests(unittest.TestCase):
    def test_doc_exists_and_mentions_websocket_endpoint(self) -> None:
        path = Path('docs/live-market-feed.md')
        self.assertTrue(path.exists())
        text = path.read_text()
        self.assertIn('wss://api.polymarket.us/v1/ws/markets', text)
        self.assertIn('shadow-first', text)
