from __future__ import annotations

import asyncio
from pathlib import Path

from polymarket_arb.ops.dashboard_refresh import refresh_dashboard_bundle
from polymarket_arb.service.supervisor_loop import run_supervisor_once


class _DemoWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        if limit > 1:
            yield {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6}


def run_capture_supervisor_once(root: str | Path, limit: int):
    root = Path(root)

    async def _run():
        result = await run_supervisor_once(
            ws_client=_DemoWSClient(),
            capture_path=root / 'capture.jsonl',
            heartbeat_path=root / 'heartbeat.json',
            metrics_path=root / 'metrics.json',
            health_path=root / 'health.json',
            alerts_path=root / 'alerts.json',
            limit=limit,
        )
        refresh_dashboard_bundle(root, {'captured': result['captured']})
        return result

    return asyncio.run(_run())
