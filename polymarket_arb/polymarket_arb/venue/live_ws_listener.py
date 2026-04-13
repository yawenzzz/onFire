from __future__ import annotations


def capture_messages(ws_client, limit: int):
    return list(ws_client.iter_messages(limit=limit))
