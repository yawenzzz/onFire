import asyncio
import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.venue.async_capture import capture_jsonl_messages


class StubAsyncWS:
    def __init__(self, messages):
        self._messages = messages

    async def iter_messages(self, limit: int):
        for idx, msg in enumerate(self._messages):
            if idx >= limit:
                break
            yield msg


class AsyncCaptureTests(unittest.TestCase):
    def test_capture_jsonl_messages_writes_lines(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp) / 'capture.jsonl'
                ws = StubAsyncWS([
                    {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5},
                    {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6},
                ])
                count = await capture_jsonl_messages(ws, path, limit=2)
                self.assertEqual(count, 2)
                lines = path.read_text().strip().splitlines()
                self.assertEqual(json.loads(lines[0])['market_id'], 'm1')
        asyncio.run(run_test())
