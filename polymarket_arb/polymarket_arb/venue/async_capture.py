from __future__ import annotations

from pathlib import Path

from polymarket_arb.data.jsonl_capture import append_jsonl


async def capture_jsonl_messages(ws_client, path: str | Path, limit: int) -> int:
    count = 0
    async for message in ws_client.iter_messages(limit=limit):
        append_jsonl(path, message)
        count += 1
    return count
