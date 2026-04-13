from __future__ import annotations

from polymarket_arb.auth.headers import build_auth_headers


def build_ws_auth_headers(access_key: str, signature: str, timestamp: str) -> dict:
    return build_auth_headers(access_key, signature, timestamp)
