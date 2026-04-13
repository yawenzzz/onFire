from __future__ import annotations

import json
from pathlib import Path

from polymarket_arb.ops.health_model import HealthStatus


def write_health_snapshot(path: str | Path, status: HealthStatus) -> None:
    Path(path).write_text(json.dumps({
        'feed_fresh': status.feed_fresh,
        'archive_ok': status.archive_ok,
        'parse_error_rate_ok': status.parse_error_rate_ok,
        'healthy': status.healthy(),
    }, indent=2, sort_keys=True))
