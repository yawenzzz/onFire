from __future__ import annotations

from pathlib import Path

from polymarket_arb.shadow.capture_dashboard import build_capture_dashboard
from polymarket_arb.shadow.capture_report import build_capture_shadow_report
from polymarket_arb.shadow.report_archive import archive_bundle


def run_capture_bundle(capture_path: str | Path, archive_root: str | Path, session_id: str, surface_id: str):
    report = build_capture_shadow_report(capture_path, session_id=session_id, surface_id=surface_id)
    summary = f"mode=shadow verdict={report['verdict']}"
    dashboard = build_capture_dashboard(capture_path)
    return archive_bundle(archive_root, session_id, report, summary, dashboard)
