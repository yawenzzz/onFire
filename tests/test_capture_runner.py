import asyncio
import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_runner import run_capture


class StubWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        if limit > 1:
            yield {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6}


class CaptureRunnerTests(unittest.TestCase):
    def test_run_capture_writes_jsonl(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp) / 'capture.jsonl'
                count = await run_capture(StubWSClient(), path, limit=2)
                self.assertEqual(count, 2)
                lines = path.read_text().strip().splitlines()
                self.assertEqual(json.loads(lines[0])['market_id'], 'm1')
        asyncio.run(run_test())
