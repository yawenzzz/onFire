from __future__ import annotations


def websocket_connect_factory():
    try:
        import websockets  # type: ignore
    except Exception:
        return None
    return websockets.connect
