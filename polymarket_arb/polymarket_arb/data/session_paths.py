from __future__ import annotations

from pathlib import Path


def session_root(root: str | Path, session_id: str) -> Path:
    return Path(root) / 'sessions' / session_id


def session_file(root: str | Path, session_id: str, filename: str) -> Path:
    return session_root(root, session_id) / filename
