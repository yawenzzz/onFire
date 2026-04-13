from __future__ import annotations

from pathlib import Path

from polymarket_arb.data.jsonl_market_loader import load_market_snapshots_jsonl


def build_capture_shadow_report(path: str | Path, session_id: str, surface_id: str) -> dict:
    snapshots = load_market_snapshots_jsonl(path)
    return {
        'session_id': session_id,
        'surface_id': surface_id,
        'snapshot_count': len(snapshots),
        'verdict': 'CERTIFICATION_INCOMPLETE',
    }
