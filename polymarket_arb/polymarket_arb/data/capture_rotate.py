from __future__ import annotations

from pathlib import Path


def rotate_capture_path(root: str | Path, session_id: str) -> Path:
    root = Path(root)
    path = root / 'sessions' / session_id / 'capture.jsonl'
    path.parent.mkdir(parents=True, exist_ok=True)
    return path
