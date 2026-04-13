from __future__ import annotations

import json
from pathlib import Path

from polymarket_arb.data.market_message_normalizer import normalize_market_message


def load_market_snapshots_jsonl(path: str | Path):
    out = []
    for line in Path(path).read_text().splitlines():
        if not line.strip():
            continue
        out.append(normalize_market_message(json.loads(line)))
    return out
