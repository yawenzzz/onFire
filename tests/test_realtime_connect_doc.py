import unittest
from pathlib import Path


class RealtimeConnectDocTests(unittest.TestCase):
    def test_doc_exists_and_mentions_websockets_install(self) -> None:
        text = Path('docs/realtime-connect-example.md').read_text()
        self.assertIn('pip install websockets', text)
        self.assertIn('wss://api.polymarket.us/v1/ws/markets', text)
        self.assertIn('additional_headers', text)
