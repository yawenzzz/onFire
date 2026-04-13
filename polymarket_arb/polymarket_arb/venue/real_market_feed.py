from __future__ import annotations

from pathlib import Path


class LocalMarketFeed:
    def __init__(self, path: str | Path, loader) -> None:
        self.path = Path(path)
        self.loader = loader

    def read(self):
        return self.loader(self.path)
