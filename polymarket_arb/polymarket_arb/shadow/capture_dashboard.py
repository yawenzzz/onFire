from __future__ import annotations

from pathlib import Path

from polymarket_arb.data.jsonl_market_loader import load_market_snapshots_jsonl


def build_capture_dashboard(path: str | Path) -> dict:
    snaps = load_market_snapshots_jsonl(path)
    count = len(snaps)
    return {
        'snapshot_count': count,
        'preview_success_rate': 0.0,
        'surface_id': 'unknown',
    }
