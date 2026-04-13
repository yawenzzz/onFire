import asyncio
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.service.supervisor_loop import run_supervisor_once


class StubWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}


class SupervisorLoopTests(unittest.TestCase):
    def test_supervisor_once_writes_capture_and_snapshots(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                result = await run_supervisor_once(
                    ws_client=StubWSClient(),
                    capture_path=root / 'capture.jsonl',
                    heartbeat_path=root / 'heartbeat.json',
                    metrics_path=root / 'metrics.json',
                    health_path=root / 'health.json',
                    alerts_path=root / 'alerts.json',
                    limit=1,
                )
                self.assertEqual(result['captured'], 1)
                self.assertTrue((root / 'metrics.json').exists())
                self.assertTrue((root / 'health.json').exists())
                self.assertTrue((root / 'alerts.json').exists())
        asyncio.run(run_test())
