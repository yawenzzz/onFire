from __future__ import annotations

from pathlib import Path
import os


def load_dotenv_file(path: str | Path) -> None:
    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        os.environ[key.strip()] = value.strip()
