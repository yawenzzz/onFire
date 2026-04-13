from __future__ import annotations

import json
from pathlib import Path

REQUIRED_KEYS = {
    'session_id',
    'surface_id',
    'outcome_count',
    'ordered_thresholds',
    'offset_relation',
    'legs',
    'pi_min_stress_usd',
    'hedge_completion_prob',
    'capital_efficiency',
    'surface_resolved',
    'jurisdiction_eligible',
}


def load_shadow_input(path: str | Path) -> dict:
    data = json.loads(Path(path).read_text())
    missing = sorted(REQUIRED_KEYS - set(data.keys()))
    if missing:
        raise ValueError(f'missing required keys: {", ".join(missing)}')
    return data
