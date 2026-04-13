from __future__ import annotations

from pathlib import Path


class LocalRuleSource:
    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)

    def read_text(self) -> str:
        return self.path.read_text()
