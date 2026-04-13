from __future__ import annotations


def build_auth_headers(access_key: str, signature: str, timestamp: str) -> dict:
    return {
        'X-PM-Access-Key': access_key,
        'X-PM-Signature': signature,
        'X-PM-Timestamp': timestamp,
    }
