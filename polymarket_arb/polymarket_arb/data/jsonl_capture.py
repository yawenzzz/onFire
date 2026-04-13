from __future__ import annotations

import json
from pathlib import Path


def append_jsonl(path: str | Path, item: dict) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('a') as f:
        f.write(json.dumps(item) + '\n')
