from __future__ import annotations

import json
from pathlib import Path


def write_heartbeat(path: str | Path, service: str, alive: bool) -> None:
    Path(path).write_text(json.dumps({'service': service, 'alive': alive}, indent=2, sort_keys=True))
