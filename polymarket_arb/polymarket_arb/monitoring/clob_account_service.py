from __future__ import annotations

from pathlib import Path

from polymarket_arb.auth.clob_runtime import detect_clob_runtime_status, load_clob_runtime_config
from polymarket_arb.monitoring.clob_account_snapshot import build_clob_account_snapshot


def load_clob_account_snapshot(root: str | Path, client_factory=None, trade_limit: int = 50, order_limit: int = 20):
    status = detect_clob_runtime_status(root)
    if status["mode"] != "account-auth-ready":
        return status, None

    cfg = load_clob_runtime_config(root)
    if cfg is None:
        failed = dict(status)
        failed["reason"] = "clob runtime config missing"
        return failed, None

    try:
        if client_factory is None:
            from py_clob_client.client import ClobClient
            from py_clob_client.clob_types import ApiCreds

            def client_factory():
                return ClobClient(
                    cfg.host,
                    chain_id=cfg.chain_id,
                    key=cfg.private_key,
                    creds=ApiCreds(
                        api_key=cfg.api_creds.api_key,
                        api_secret=cfg.api_creds.api_secret,
                        api_passphrase=cfg.api_creds.api_passphrase,
                    ),
                    signature_type=cfg.signature_type,
                    funder=cfg.funder,
                )

        snapshot = build_clob_account_snapshot(client_factory(), trade_limit=trade_limit, order_limit=order_limit)
    except Exception as exc:
        failed = dict(status)
        failed["reason"] = f"clob account api failed: {type(exc).__name__}: {exc}"
        return failed, None

    live = dict(status)
    live["mode"] = "account-live"
    live["reason"] = "clob account api connected"
    return live, snapshot
