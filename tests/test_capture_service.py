import asyncio
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.service.capture_service import run_capture_cycle


class StubWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}


class CaptureServiceTests(unittest.TestCase):
    def test_run_capture_cycle_writes_capture_and_heartbeat(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                result = await run_capture_cycle(
                    ws_client=StubWSClient(),
                    capture_path=root / 'capture.jsonl',
                    heartbeat_path=root / 'heartbeat.json',
                    limit=1,
                )
                self.assertEqual(result['captured'], 1)
                self.assertTrue((root / 'capture.jsonl').exists())
                self.assertTrue((root / 'heartbeat.json').exists())
        asyncio.run(run_test())
