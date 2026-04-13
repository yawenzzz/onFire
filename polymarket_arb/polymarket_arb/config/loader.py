from __future__ import annotations

import json
from pathlib import Path

from polymarket_arb.config.schemas import LaunchGate


def load_launch_gate(path: str | Path) -> LaunchGate:
    data = json.loads(Path(path).read_text())
    return LaunchGate(**data)
