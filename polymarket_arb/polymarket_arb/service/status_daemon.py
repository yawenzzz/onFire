from __future__ import annotations

from pathlib import Path

from polymarket_arb.service.status_service import start_status_service


class StatusDaemon:
    def __init__(self, server) -> None:
        self.server = server

    def stop(self) -> None:
        self.server.shutdown()
        self.server.server_close()


def start_status_daemon(root: str | Path, host: str = '127.0.0.1', port: int = 0) -> StatusDaemon:
    return StatusDaemon(start_status_service(root=root, host=host, port=port))
