import unittest
from pathlib import Path


class RealtimeIngestionDocTests(unittest.TestCase):
    def test_doc_exists_and_mentions_websocket_and_shadow_first(self) -> None:
        text = Path('docs/realtime-ingestion.md').read_text()
        self.assertIn('wss://api.polymarket.us/v1/ws/markets', text)
        self.assertIn('shadow-first', text)
        self.assertIn('capture jsonl', text)
