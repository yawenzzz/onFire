from __future__ import annotations

from pathlib import Path

from polymarket_arb.app.capture_runner import run_capture
from polymarket_arb.ops.daemon_heartbeat import write_heartbeat


async def run_capture_cycle(ws_client, capture_path: str | Path, heartbeat_path: str | Path, limit: int) -> dict:
    captured = await run_capture(ws_client, capture_path, limit)
    write_heartbeat(heartbeat_path, service='capture', alive=True)
    return {'captured': captured}
