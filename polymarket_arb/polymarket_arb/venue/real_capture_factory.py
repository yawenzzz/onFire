from __future__ import annotations

from polymarket_arb.venue.async_websocket_client import AsyncWebSocketClient


def build_async_ws_client(url: str, connect_factory):
    if connect_factory is None:
        return None
    return AsyncWebSocketClient(url=url, connect_impl=connect_factory)
