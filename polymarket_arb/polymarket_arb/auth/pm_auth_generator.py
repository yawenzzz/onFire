from __future__ import annotations

import base64
import time

from cryptography.hazmat.primitives.asymmetric import ed25519


def _decode_private_key_seed(private_key_base64: str) -> bytes:
    normalized = private_key_base64.strip()
    padding = (-len(normalized)) % 4
    if padding:
        normalized = normalized + ("=" * padding)
    try:
        raw = base64.b64decode(normalized)
    except Exception as exc:  # pragma: no cover - exercised via ValueError contract
        try:
            raw = base64.urlsafe_b64decode(normalized)
        except Exception as inner_exc:  # pragma: no cover - exercised via ValueError contract
            raise ValueError("invalid base64 private key") from inner_exc

    if len(raw) < 32:
        raise ValueError("private key must contain at least 32 bytes")
    return raw[:32]


def generate_pm_auth_exports(
    access_key: str,
    private_key_base64: str,
    path: str = "/v1/ws/markets",
    method: str = "GET",
    timestamp_ms: str | None = None,
) -> dict[str, str]:
    timestamp_ms = timestamp_ms or str(int(time.time() * 1000))
    seed = _decode_private_key_seed(private_key_base64)
    private_key = ed25519.Ed25519PrivateKey.from_private_bytes(seed)
    message = f"{timestamp_ms}{method}{path}".encode("utf-8")
    signature = base64.b64encode(private_key.sign(message)).decode("utf-8")
    return {
        "PM_ACCESS_KEY": access_key,
        "PM_TIMESTAMP": timestamp_ms,
        "PM_SIGNATURE": signature,
    }
