from __future__ import annotations

import json
from pathlib import Path


def write_json_report(path: str | Path, report: dict) -> None:
    path = Path(path)
    path.write_text(json.dumps(report, indent=2, sort_keys=True))
