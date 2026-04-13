from __future__ import annotations

import importlib.util
import os
from dataclasses import dataclass
from pathlib import Path

from polymarket_arb.auth.account_status import _read_env_file
from polymarket_arb.auth.clob_creds import ClobApiCreds


@dataclass
class ClobRuntimeConfig:
    private_key: str
    api_creds: ClobApiCreds
    host: str = "https://clob.polymarket.com"
    chain_id: int = 137
    signature_type: int = 0
    funder: str | None = None
    signer_address: str | None = None


def _env_from_root(root: str | Path) -> dict[str, str]:
    root = Path(root)
    env = dict(os.environ)
    env.update(_read_env_file(root / ".env"))
    env.update(_read_env_file(root / ".env.local"))
    return env


def _signer_address_from_private_key(private_key: str | None) -> str | None:
    if not private_key:
        return None
    try:
        from eth_account import Account

        normalized = private_key if private_key.startswith("0x") else f"0x{private_key}"
        return Account.from_key(normalized).address.lower()
    except Exception:
        return None


def load_clob_runtime_config(root: str | Path) -> ClobRuntimeConfig | None:
    env = _env_from_root(root)
    private_key = env.get("PRIVATE_KEY") or env.get("CLOB_PRIVATE_KEY")
    api_key = env.get("CLOB_API_KEY")
    api_secret = env.get("CLOB_SECRET")
    api_passphrase = env.get("CLOB_PASS_PHRASE")
    if not private_key or not api_key or not api_secret or not api_passphrase:
        return None
    return ClobRuntimeConfig(
        private_key=private_key,
        api_creds=ClobApiCreds(
            api_key=api_key,
            api_secret=api_secret,
            api_passphrase=api_passphrase,
        ),
        host=env.get("CLOB_HOST", "https://clob.polymarket.com"),
        chain_id=int(env.get("CHAIN_ID", "137")),
        signature_type=int(env.get("SIGNATURE_TYPE", "0")),
        funder=env.get("FUNDER_ADDRESS") or env.get("FUNDER"),
        signer_address=_signer_address_from_private_key(private_key),
    )


def detect_clob_runtime_status(root: str | Path) -> dict:
    env = _env_from_root(root)
    creds_present = all(env.get(key) for key in ("CLOB_API_KEY", "CLOB_SECRET", "CLOB_PASS_PHRASE"))
    private_key_present = bool(env.get("PRIVATE_KEY") or env.get("CLOB_PRIVATE_KEY"))
    signer_address = _signer_address_from_private_key(env.get("PRIVATE_KEY") or env.get("CLOB_PRIVATE_KEY"))
    sdk_available = importlib.util.find_spec("py_clob_client") is not None
    signature_type = int(env.get("SIGNATURE_TYPE", "0"))
    funder_present = bool(env.get("FUNDER_ADDRESS") or env.get("FUNDER"))
    private_key_var = "PRIVATE_KEY" if env.get("PRIVATE_KEY") else "CLOB_PRIVATE_KEY" if env.get("CLOB_PRIVATE_KEY") else None

    if creds_present and private_key_present and sdk_available:
        return {
            "mode": "account-auth-ready",
            "creds_present": True,
            "sdk_available": True,
            "private_key_present": True,
            "private_key_var": private_key_var,
            "signer_address": signer_address,
            "funder_address": env.get("FUNDER_ADDRESS") or env.get("FUNDER"),
            "signature_type": signature_type,
            "funder_present": funder_present,
            "reason": "clob level2 auth present",
        }
    if creds_present:
        reason = (
            "missing private key for clob level2 auth (set PRIVATE_KEY or CLOB_PRIVATE_KEY)"
            if not private_key_present
            else "py_clob_client unavailable"
        )
        return {
            "mode": "account-ready",
            "creds_present": True,
            "sdk_available": sdk_available,
            "private_key_present": private_key_present,
            "private_key_var": private_key_var,
            "signer_address": signer_address,
            "funder_address": env.get("FUNDER_ADDRESS") or env.get("FUNDER"),
            "signature_type": signature_type,
            "funder_present": funder_present,
            "reason": reason,
        }
    return {
        "mode": "public-only",
        "creds_present": False,
        "sdk_available": sdk_available,
        "private_key_present": private_key_present,
        "private_key_var": private_key_var,
        "signer_address": signer_address,
        "funder_address": env.get("FUNDER_ADDRESS") or env.get("FUNDER"),
        "signature_type": signature_type,
        "funder_present": funder_present,
        "reason": "missing CLOB credentials",
    }
