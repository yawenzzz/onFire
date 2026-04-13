from __future__ import annotations

import importlib.util
import os
from pathlib import Path

from polymarket_arb.auth.pm_auth_generator import generate_pm_auth_exports


def _read_env_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def detect_account_status(root: str | Path, sdk_available: bool | None = None) -> dict:
    root = Path(root)
    env = dict(os.environ)
    env.update(_read_env_file(root / ".env"))
    env.update(_read_env_file(root / ".env.local"))

    creds_present = all(
        env.get(key)
        for key in ("CLOB_API_KEY", "CLOB_SECRET", "CLOB_PASS_PHRASE")
    )
    pm_auth_present = all(
        env.get(key)
        for key in ("PM_ACCESS_KEY", "PM_SIGNATURE", "PM_TIMESTAMP")
    ) or all(
        env.get(key)
        for key in ("POLYMARKET_KEY_ID", "POLYMARKET_SECRET_KEY")
    )
    if sdk_available is None:
        sdk_available = importlib.util.find_spec("py_clob_client") is not None

    if pm_auth_present:
        return {
            "mode": "account-auth-ready",
            "creds_present": creds_present,
            "sdk_available": sdk_available,
            "pm_auth_present": True,
            "reason": "private account api auth present",
        }

    if not creds_present:
        return {
            "mode": "public-only",
            "creds_present": False,
            "sdk_available": sdk_available,
            "pm_auth_present": False,
            "reason": "missing CLOB credentials",
        }

    if not sdk_available:
        return {
            "mode": "account-ready",
            "creds_present": True,
            "sdk_available": False,
            "pm_auth_present": False,
            "reason": "sdk unavailable",
        }

    return {
        "mode": "account-ready",
        "creds_present": True,
        "sdk_available": True,
        "pm_auth_present": False,
        "reason": "private account API not wired yet",
    }


def load_pm_auth_from_root(root: str | Path) -> dict | None:
    root = Path(root)
    env = dict(os.environ)
    env.update(_read_env_file(root / ".env"))
    env.update(_read_env_file(root / ".env.local"))
    long_lived_access_key = env.get("POLYMARKET_KEY_ID")
    long_lived_secret = env.get("POLYMARKET_SECRET_KEY")
    if long_lived_access_key and long_lived_secret:
        try:
            generated = generate_pm_auth_exports(
                access_key=long_lived_access_key,
                private_key_base64=long_lived_secret,
            )
            return {
                "access_key": generated["PM_ACCESS_KEY"],
                "signature": generated["PM_SIGNATURE"],
                "timestamp": generated["PM_TIMESTAMP"],
            }
        except Exception:
            pass
    access_key = env.get("PM_ACCESS_KEY")
    signature = env.get("PM_SIGNATURE")
    timestamp = env.get("PM_TIMESTAMP")
    if not access_key or not signature or not timestamp:
        return None
    return {
        "access_key": access_key,
        "signature": signature,
        "timestamp": timestamp,
    }
