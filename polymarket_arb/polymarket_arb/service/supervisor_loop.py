from __future__ import annotations

from pathlib import Path

from polymarket_arb.ops.alert_rules import evaluate_alert_rules
from polymarket_arb.ops.alert_snapshot import write_alert_snapshot
from polymarket_arb.ops.daemon_heartbeat import write_heartbeat
from polymarket_arb.ops.dashboard_bundle import write_dashboard_bundle
from polymarket_arb.ops.health_model import HealthStatus
from polymarket_arb.ops.health_snapshot import write_health_snapshot
from polymarket_arb.ops.metrics_emitter import build_metrics_payload
from polymarket_arb.ops.metrics_snapshot import write_metrics_snapshot
from polymarket_arb.service.capture_service import run_capture_cycle
from polymarket_arb.shadow.metrics import ShadowMetrics


async def run_supervisor_once(ws_client, capture_path: str | Path, heartbeat_path: str | Path, metrics_path: str | Path, health_path: str | Path, alerts_path: str | Path, limit: int):
    result = await run_capture_cycle(ws_client, capture_path=capture_path, heartbeat_path=heartbeat_path, limit=limit)
    metrics = ShadowMetrics.from_counts(
        session_id='supervisor',
        surface_id='unknown',
        preview_successes=result['captured'],
        preview_attempts=result['captured'],
        hedge_completions=0,
        hedge_attempts=0,
        false_positives=0,
        candidate_count=result['captured'],
        api_429_count=0,
        reconciliation_mismatch_count=0,
    )
    payload = build_metrics_payload(metrics, reconnect_count=0, parse_error_count=0)
    write_metrics_snapshot(metrics_path, payload)
    status = HealthStatus(feed_fresh=True, archive_ok=True, parse_error_rate_ok=True)
    write_health_snapshot(health_path, status)
    alerts = evaluate_alert_rules(status)
    write_alert_snapshot(alerts_path, alerts)
    write_dashboard_bundle(Path(metrics_path).parent, payload)
    write_heartbeat(heartbeat_path, service='capture', alive=True)
    return result
