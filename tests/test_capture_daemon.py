import asyncio
import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_daemon import run_capture_daemon_once


class StubWS:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        if limit > 1:
            yield {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6}


class CaptureDaemonTests(unittest.TestCase):
    def test_daemon_once_writes_capture_file(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                out = Path(tmp) / 'capture.jsonl'
                count = await run_capture_daemon_once(StubWS(), out, limit=2)
                self.assertEqual(count, 2)
                self.assertEqual(json.loads(out.read_text().splitlines()[1])['market_id'], 'm2')
        asyncio.run(run_test())
