from __future__ import annotations

from pathlib import Path

from polymarket_arb.ops.http_status_server import start_status_server


def start_status_service(root: str | Path, host: str = '127.0.0.1', port: int = 0):
    return start_status_server(root=root, host=host, port=port)
