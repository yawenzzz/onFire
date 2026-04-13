from __future__ import annotations

import json
from pathlib import Path


def _session_dir(root: str | Path, session_id: str) -> Path:
    root = Path(root)
    return root / 'sessions' / session_id


def archive_report(root: str | Path, session_id: str, report: dict) -> Path:
    path = _session_dir(root, session_id) / 'certification-report.json'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True))
    return path


def archive_bundle(root: str | Path, session_id: str, report: dict, summary: str, dashboard: dict):
    session_dir = _session_dir(root, session_id)
    session_dir.mkdir(parents=True, exist_ok=True)
    report_path = session_dir / 'certification-report.json'
    summary_path = session_dir / 'summary.txt'
    dashboard_path = session_dir / 'dashboard.json'
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True))
    summary_path.write_text(summary)
    dashboard_path.write_text(json.dumps(dashboard, indent=2, sort_keys=True))
    return report_path, summary_path, dashboard_path
