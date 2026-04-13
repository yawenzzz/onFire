from __future__ import annotations

import json


def render_metrics_response(payload: dict) -> str:
    return json.dumps(payload, sort_keys=True)
