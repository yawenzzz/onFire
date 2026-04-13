#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import os
import socket
import ssl


def tcp_probe(host: str, port: int, timeout: float) -> None:
    with socket.create_connection((host, port), timeout=timeout) as sock:
        print("tcp_ok", sock.getpeername())


def tls_probe(host: str, port: int, timeout: float) -> None:
    ctx = ssl.create_default_context()
    with socket.create_connection((host, port), timeout=timeout) as sock:
        with ctx.wrap_socket(sock, server_hostname=host) as ssock:
            print("tls_ok", ssock.version(), ssock.cipher())


def websocket_upgrade_probe(host: str, path: str, timeout: float, origin: str | None) -> None:
    key = base64.b64encode(os.urandom(16)).decode()
    req = (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {host}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        "User-Agent: pm-ws-handshake-probe/0.1\r\n"
        + (f"Origin: {origin}\r\n" if origin else "")
        + "\r\n"
    ).encode()
    ctx = ssl.create_default_context()
    with socket.create_connection((host, 443), timeout=timeout) as sock:
        with ctx.wrap_socket(sock, server_hostname=host) as ssock:
            ssock.settimeout(timeout)
            ssock.sendall(req)
            data = ssock.recv(4096)
            print(data.decode("utf-8", errors="replace")[:1000])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="ws-subscriptions-clob.polymarket.com")
    parser.add_argument("--path", default="/ws/market")
    parser.add_argument("--timeout-seconds", type=float, default=5.0)
    parser.add_argument("--origin", default=None)
    args = parser.parse_args(argv)

    try:
        tcp_probe(args.host, 443, args.timeout_seconds)
    except Exception as exc:
        print(f"tcp_err: {type(exc).__name__}: {exc}")
        return 1

    try:
        tls_probe(args.host, 443, args.timeout_seconds)
    except Exception as exc:
        print(f"tls_err: {type(exc).__name__}: {exc}")
        return 1

    try:
        websocket_upgrade_probe(args.host, args.path, args.timeout_seconds, args.origin)
    except Exception as exc:
        print(f"upgrade_err: {type(exc).__name__}: {exc}")
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
