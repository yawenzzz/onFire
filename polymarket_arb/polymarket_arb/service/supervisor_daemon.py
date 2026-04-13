from __future__ import annotations

import asyncio
from pathlib import Path

from polymarket_arb.service.supervisor_loop import run_supervisor_once


async def run_supervisor_daemon(root: str | Path, ws_client, limit: int, iterations: int, sleep_seconds: float):
    root = Path(root)
    completed = 0
    for _ in range(iterations):
        await run_supervisor_once(
            ws_client=ws_client,
            capture_path=root / 'capture.jsonl',
            heartbeat_path=root / 'heartbeat.json',
            metrics_path=root / 'metrics.json',
            health_path=root / 'health.json',
            alerts_path=root / 'alerts.json',
            limit=limit,
        )
        completed += 1
        if sleep_seconds:
            await asyncio.sleep(sleep_seconds)
    return completed
