from __future__ import annotations

import json
from pathlib import Path


def write_dashboard_bundle(root: str | Path, dashboard: dict) -> Path:
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    path = root / 'dashboard.json'
    path.write_text(json.dumps(dashboard, indent=2, sort_keys=True))
    return path
