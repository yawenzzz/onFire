from __future__ import annotations

import json
from pathlib import Path

from polymarket_arb.data.contracts import MarketSnapshot


def load_market_snapshots(path: str | Path) -> list[MarketSnapshot]:
    raw = json.loads(Path(path).read_text())
    return [MarketSnapshot(**item) for item in raw]
