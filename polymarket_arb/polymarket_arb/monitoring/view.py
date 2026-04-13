from __future__ import annotations

from polymarket_arb.app.monitor_tui import build_monitor_lines
from polymarket_arb.monitoring.models import MonitorSnapshot


def _fit(line: str, width: int) -> str:
    if len(line) <= width:
        return line
    if width <= 3:
        return line[:width]
    return line[: width - 3] + "..."


def render_monitor_lines(snapshot: MonitorSnapshot, width: int, height: int) -> list[str]:
    return build_monitor_lines(snapshot, width=width, height=height)
