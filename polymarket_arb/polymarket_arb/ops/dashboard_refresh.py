from __future__ import annotations

from pathlib import Path

from polymarket_arb.ops.dashboard_bundle import write_dashboard_bundle


def refresh_dashboard_bundle(root: str | Path, dashboard: dict):
    return write_dashboard_bundle(root, dashboard)
