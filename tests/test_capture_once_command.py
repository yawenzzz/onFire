import asyncio
import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_once import run_capture_once


class StubWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}


class CaptureOnceCommandTests(unittest.TestCase):
    def test_run_capture_once_writes_capture_file(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                out = Path(tmp) / 'capture.jsonl'
                count = await run_capture_once(StubWSClient(), out, limit=1)
                self.assertEqual(count, 1)
                self.assertEqual(json.loads(out.read_text().strip())['market_id'], 'm1')
        asyncio.run(run_test())
