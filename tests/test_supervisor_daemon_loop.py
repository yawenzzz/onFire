import asyncio
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.service.supervisor_daemon import run_supervisor_daemon


class StubWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}


class SupervisorDaemonLoopTests(unittest.TestCase):
    def test_runs_multiple_iterations(self) -> None:
        async def run_test():
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                iterations = await run_supervisor_daemon(root=root, ws_client=StubWSClient(), limit=1, iterations=2, sleep_seconds=0)
                self.assertEqual(iterations, 2)
                self.assertTrue((root / 'metrics.json').exists())
        asyncio.run(run_test())
