from __future__ import annotations


def rotated_capture_name(prefix: str, suffix: str, sequence: int) -> str:
    return f'{prefix}-{sequence:04d}.{suffix}'
