from __future__ import annotations


def compute_backoff_seconds(attempt: int, base: float = 0.5, cap: float = 30.0) -> float:
    if attempt < 1:
        attempt = 1
    value = base * (2 ** (attempt - 1))
    return min(value, cap)
