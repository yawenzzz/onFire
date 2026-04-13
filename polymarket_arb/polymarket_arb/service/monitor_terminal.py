from __future__ import annotations

from polymarket_arb.monitoring.models import MonitorSnapshot
from polymarket_arb.monitoring.view import render_monitor_lines


def build_terminal_lines(snapshot: MonitorSnapshot, width: int = 100) -> list[str]:
    return render_monitor_lines(snapshot, width=width, height=24)
