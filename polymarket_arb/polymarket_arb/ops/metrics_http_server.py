from __future__ import annotations

import json
from pathlib import Path


def _read_json(path: Path):
    return json.loads(path.read_text()) if path.exists() else {}


def build_status_payload(root: str | Path) -> dict:
    root = Path(root)
    return {
        'metrics': _read_json(root / 'metrics.json'),
        'health': _read_json(root / 'health.json'),
        'alerts': _read_json(root / 'alerts.json'),
    }
