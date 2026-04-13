from __future__ import annotations

import os


def load_ws_auth_from_env():
    access_key = os.environ.get('PM_ACCESS_KEY')
    signature = os.environ.get('PM_SIGNATURE')
    timestamp = os.environ.get('PM_TIMESTAMP')
    if not access_key or not signature or not timestamp:
        return None
    return {
        'access_key': access_key,
        'signature': signature,
        'timestamp': timestamp,
    }


from polymarket_arb.auth.config import WsAuthConfig

def load_ws_auth_config_from_env() -> WsAuthConfig | None:
    raw = load_ws_auth_from_env()
    if raw is None:
        return None
    return WsAuthConfig(access_key=raw['access_key'], signature=raw['signature'], timestamp=raw['timestamp'])
