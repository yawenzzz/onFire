from __future__ import annotations

from polymarket_arb.auth.config import WsAuthConfig


def validate_ws_auth_config(cfg: WsAuthConfig) -> list[str]:
    problems = []
    if not cfg.access_key:
        problems.append('missing access key')
    if not cfg.signature:
        problems.append('missing signature')
    if not cfg.timestamp:
        problems.append('missing timestamp')
    return problems
