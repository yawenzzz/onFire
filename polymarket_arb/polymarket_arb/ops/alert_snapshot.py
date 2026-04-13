from __future__ import annotations

import json
from pathlib import Path


def write_alert_snapshot(path: str | Path, alerts: list[str]) -> None:
    Path(path).write_text(json.dumps({'alerts': alerts}, indent=2, sort_keys=True))
