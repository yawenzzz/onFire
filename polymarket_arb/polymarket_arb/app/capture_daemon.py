from __future__ import annotations

from pathlib import Path

from polymarket_arb.app.capture_runner import run_capture


async def run_capture_daemon_once(ws_client, output: str | Path, limit: int) -> int:
    return await run_capture(ws_client, output, limit)
