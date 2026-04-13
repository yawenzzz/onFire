from __future__ import annotations

import json
from pathlib import Path


def write_metrics_snapshot(path: str | Path, payload: dict) -> None:
    Path(path).write_text(json.dumps(payload, indent=2, sort_keys=True))
