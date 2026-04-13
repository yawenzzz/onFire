from __future__ import annotations

from pathlib import Path

from polymarket_arb.app.capture_bundle import run_capture_bundle


def run_sample_to_shadow_bundle(capture_path: str | Path, archive_root: str | Path, session_id: str, surface_id: str):
    return run_capture_bundle(
        capture_path=capture_path,
        archive_root=archive_root,
        session_id=session_id,
        surface_id=surface_id,
    )
