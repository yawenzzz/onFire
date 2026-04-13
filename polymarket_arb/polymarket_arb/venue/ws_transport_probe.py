from __future__ import annotations

import socket
import ssl
from urllib.parse import urlparse


def probe_ws_transport(ws_url: str, timeout_seconds: float = 5.0) -> tuple[bool, str]:
    parsed = urlparse(ws_url)
    host = parsed.hostname
    port = parsed.port or 443
    if not host:
        return False, "invalid websocket url"
    try:
        with socket.create_connection((host, port), timeout=timeout_seconds) as sock:
            pass
    except Exception as exc:
        return False, f"tcp_err: {type(exc).__name__}: {exc}"

    ctx = ssl.create_default_context()
    try:
        with socket.create_connection((host, port), timeout=timeout_seconds) as sock:
            with ctx.wrap_socket(sock, server_hostname=host):
                return True, "tls_ok"
    except Exception as exc:
        return False, f"tls_err: {type(exc).__name__}: {exc}"
