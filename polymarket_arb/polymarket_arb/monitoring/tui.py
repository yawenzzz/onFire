from __future__ import annotations

from polymarket_arb.app.monitor_tui import build_monitor_lines


def render_snapshot_lines(
    snapshot,
    width: int = 120,
    height: int = 30,
    watched_offset: int = 0,
    account_section: str = "orders",
    account_offsets: dict | None = None,
) -> list[str]:
    return build_monitor_lines(
        snapshot,
        width=width,
        height=height,
        watched_offset=watched_offset,
        account_section=account_section,
        account_offsets=account_offsets,
    )
