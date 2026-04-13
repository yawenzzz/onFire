from __future__ import annotations

from pathlib import Path

from polymarket_arb.venue.async_capture import capture_jsonl_messages


async def run_capture(ws_client, path: str | Path, limit: int) -> int:
    return await capture_jsonl_messages(ws_client, path, limit)
