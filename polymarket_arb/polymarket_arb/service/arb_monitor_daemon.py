from __future__ import annotations

import asyncio
import json
from pathlib import Path

from polymarket_arb.monitoring.gamma_client import GammaMarketClient
from polymarket_arb.monitoring.scanner import MonitorSettings, scan_events_snapshot
from polymarket_arb.ops.alert_snapshot import write_alert_snapshot
from polymarket_arb.ops.dashboard_bundle import write_dashboard_bundle
from polymarket_arb.ops.health_model import HealthStatus
from polymarket_arb.ops.health_snapshot import write_health_snapshot
from polymarket_arb.ops.metrics_snapshot import write_metrics_snapshot


def _build_metrics_payload(snapshot) -> dict:
    return {
        "iteration": snapshot.iteration,
        "event_count": snapshot.event_count,
        "complete_event_count": snapshot.complete_event_count,
        "candidate_count": snapshot.candidate_count,
        "best_gross_edge": snapshot.best_gross_edge,
        "best_cost_adjusted_edge": snapshot.best_cost_adjusted_edge,
        "scan_duration_seconds": round(snapshot.scan_duration_seconds, 3),
        "rejection_counts": snapshot.rejection_counts,
        "data_source": snapshot.data_source,
        "status": snapshot.status,
    }


def _build_alerts(snapshot) -> list[str]:
    alerts: list[str] = []
    if snapshot.status != "ok":
        alerts.append("data_source_not_ok")
    if snapshot.scan_duration_seconds > 5.0:
        alerts.append("scan_duration_seconds > 5.0")
    return alerts


async def run_arb_monitor_daemon(
    root: str | Path,
    client=None,
    event_limit: int = 50,
    scan_interval_seconds: float = 3.0,
    iterations: int = 0,
    fee_buffer: float = 0.02,
    slippage_buffer: float = 0.005,
    safety_buffer: float = 0.005,
) -> int:
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    client = client or GammaMarketClient()
    completed = 0
    iteration = 1

    while iterations <= 0 or completed < iterations:
        snapshot = scan_events_snapshot(
            client=client,
            settings=MonitorSettings(
                limit=event_limit,
                fee_buffer=fee_buffer,
                slippage_buffer=slippage_buffer,
                safety_buffer=safety_buffer,
                min_edge=fee_buffer + slippage_buffer + safety_buffer,
            ),
            iteration=iteration,
        )
        metrics_payload = _build_metrics_payload(snapshot)
        write_metrics_snapshot(root / "metrics.json", metrics_payload)
        write_health_snapshot(
            root / "health.json",
            HealthStatus(feed_fresh=snapshot.status == "ok", archive_ok=True, parse_error_rate_ok=True),
        )
        write_alert_snapshot(root / "alerts.json", _build_alerts(snapshot))
        write_dashboard_bundle(
            root,
            {
                "top_candidates": [
                    {
                        "title": candidate.title,
                        "slug": candidate.slug,
                        "gross_edge": candidate.gross_edge,
                        "cost_adjusted_edge": candidate.cost_adjusted_edge,
                        "template_type": candidate.template_type,
                    }
                    for candidate in snapshot.candidates
                ],
                **metrics_payload,
            },
        )
        completed += 1
        iteration += 1
        if iterations > 0 and completed >= iterations:
            break
        if scan_interval_seconds:
            await asyncio.sleep(scan_interval_seconds)

    return completed
