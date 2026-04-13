from __future__ import annotations

import time
from pathlib import Path
from typing import Any

from polymarket_arb.ops.alert_snapshot import write_alert_snapshot
from polymarket_arb.ops.dashboard_bundle import write_dashboard_bundle
from polymarket_arb.ops.health_model import HealthStatus
from polymarket_arb.ops.health_snapshot import write_health_snapshot
from polymarket_arb.ops.metrics_snapshot import write_metrics_snapshot


def _read(obj: Any, key: str, default=None):
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def _snapshot_dict(snapshot: Any) -> dict[str, Any]:
    if isinstance(snapshot, dict):
        return snapshot
    return {
        "view": _read(snapshot, "view"),
        "iteration": _read(snapshot, "iteration"),
        "updated_at": _read(snapshot, "updated_at"),
        "scan_latency_ms": _read(snapshot, "scan_latency_ms"),
        "target_interval_seconds": _read(snapshot, "target_interval_seconds"),
        "actual_cycle_ms": _read(snapshot, "actual_cycle_ms"),
        "scan_limit": _read(snapshot, "scan_limit"),
        "window_limit": _read(snapshot, "window_limit"),
        "scan_offset": _read(snapshot, "scan_offset"),
        "hotspot_offset": _read(snapshot, "hotspot_offset"),
        "scan_mode": _read(snapshot, "scan_mode"),
        "hot_limit": _read(snapshot, "hot_limit"),
        "full_scan_every": _read(snapshot, "full_scan_every"),
        "hot_window_count": _read(snapshot, "hot_window_count"),
        "covered_window_count": _read(snapshot, "covered_window_count"),
        "realtime_status": _read(snapshot, "realtime_status"),
        "realtime_reason": _read(snapshot, "realtime_reason"),
        "reference_candidate_count": _read(snapshot, "reference_candidate_count"),
        "reference_best_gross_edge": _read(snapshot, "reference_best_gross_edge"),
        "page_count": _read(snapshot, "page_count"),
        "event_count": _read(snapshot, "event_count", _read(snapshot, "scanned_event_count", 0)),
        "complete_event_count": _read(snapshot, "complete_event_count", 0),
        "data_source": _read(snapshot, "data_source", "unknown"),
        "account_status": _read(snapshot, "account_status", {}),
        "account_snapshot": _read(snapshot, "account_snapshot", {}),
        "reference_order_draft": _read(snapshot, "reference_order_draft"),
        "source_status": _read(snapshot, "source_status", _read(snapshot, "status", "unknown")),
        "candidate_count": _read(snapshot, "candidate_count", 0),
        "rejection_count": len(_read(snapshot, "rejections", []) or []),
        "best_gross_edge": _read(snapshot, "best_gross_edge", 0.0),
        "best_adjusted_edge": _read(snapshot, "best_cost_adjusted_edge", 0.0),
        "category_counts": _read(snapshot, "category_counts", {}),
        "structure_counts": _read(snapshot, "structure_counts", {}),
        "candidates": [
            {
                "title": _read(candidate, "title"),
                "gross_edge": _read(candidate, "gross_edge", 0.0),
                "adjusted_edge": _read(candidate, "cost_adjusted_edge", 0.0),
                "reason": _read(candidate, "rejection_reason"),
                "category": _read(candidate, "category", "unknown"),
                "best_bid": _read(candidate, "best_bid"),
                "best_ask": _read(candidate, "best_ask"),
                "last_trade_price": _read(candidate, "last_trade_price"),
                "volume_24hr": _read(candidate, "volume_24hr"),
                "liquidity": _read(candidate, "liquidity"),
                "depth_bid_top": _read(candidate, "depth_bid_top"),
                "depth_ask_top": _read(candidate, "depth_ask_top"),
                "repeat_interval_ms": _read(candidate, "repeat_interval_ms"),
                "recommended_orders": _read(candidate, "recommended_orders", []),
            }
            for candidate in (_read(snapshot, "candidates", []) or [])
        ],
        "rejections": [
            {
                "title": _read(rejection, "title"),
                "reason": _read(rejection, "rejection_reason"),
                "category": _read(rejection, "category", "unknown"),
                "gross_edge": _read(rejection, "gross_edge", 0.0),
                "adjusted_edge": _read(rejection, "cost_adjusted_edge", 0.0),
            }
            for rejection in (_read(snapshot, "rejections", []) or [])
        ],
        "watched_events": _read(snapshot, "watched_events", []),
        "rejection_reason_counts": _read(snapshot, "rejection_counts", {}),
    }


def _metrics_payload(snapshot: Any) -> dict[str, Any]:
    return {
        "view": _read(snapshot, "view"),
        "updated_at": _read(snapshot, "updated_at"),
        "iteration": _read(snapshot, "iteration"),
        "event_count": _read(snapshot, "event_count", _read(snapshot, "scanned_event_count", 0)),
        "complete_event_count": _read(snapshot, "complete_event_count", 0),
        "data_source": _read(snapshot, "data_source", "unknown"),
        "account_status": _read(snapshot, "account_status", {}),
        "account_snapshot": _read(snapshot, "account_snapshot", {}),
        "reference_order_draft": _read(snapshot, "reference_order_draft"),
        "source_status": _read(snapshot, "source_status", _read(snapshot, "status", "unknown")),
        "scan_latency_ms": _read(snapshot, "scan_latency_ms", 0),
        "target_interval_seconds": _read(snapshot, "target_interval_seconds"),
        "actual_cycle_ms": _read(snapshot, "actual_cycle_ms"),
        "scan_limit": _read(snapshot, "scan_limit"),
        "window_limit": _read(snapshot, "window_limit"),
        "scan_offset": _read(snapshot, "scan_offset"),
        "hotspot_offset": _read(snapshot, "hotspot_offset"),
        "scan_mode": _read(snapshot, "scan_mode"),
        "hot_limit": _read(snapshot, "hot_limit"),
        "full_scan_every": _read(snapshot, "full_scan_every"),
        "hot_window_count": _read(snapshot, "hot_window_count"),
        "covered_window_count": _read(snapshot, "covered_window_count"),
        "realtime_status": _read(snapshot, "realtime_status"),
        "realtime_reason": _read(snapshot, "realtime_reason"),
        "reference_candidate_count": _read(snapshot, "reference_candidate_count"),
        "reference_best_gross_edge": _read(snapshot, "reference_best_gross_edge"),
        "page_count": _read(snapshot, "page_count"),
        "candidate_count": _read(snapshot, "candidate_count", 0),
        "rejection_count": _read(snapshot, "rejection_count", len(_read(snapshot, "rejections", []) or [])),
        "best_gross_edge": _read(snapshot, "best_gross_edge", 0.0),
        "best_adjusted_edge": _read(snapshot, "best_adjusted_edge", _read(snapshot, "best_cost_adjusted_edge", 0.0)),
    }


def _alerts(snapshot: Any) -> list[str]:
    alerts: list[str] = []
    if _read(snapshot, "source_status", _read(snapshot, "status", "unknown")) != "ok":
        alerts.append("ALERT_SOURCE_NOT_OK")
    return alerts


def run_monitor_daemon(
    root: str | Path,
    scanner,
    iterations: int,
    sleep_seconds: float,
    ui=None,
    sleep_fn=time.sleep,
) -> int:
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)

    completed = 0
    last_scan_started_at: float | None = None
    try:
        while iterations <= 0 or completed < iterations:
            scan_started_at = time.time()
            snapshot = _snapshot_dict(scanner.scan_once())
            snapshot["target_interval_seconds"] = sleep_seconds
            snapshot["actual_cycle_ms"] = (
                int((scan_started_at - last_scan_started_at) * 1000)
                if last_scan_started_at is not None
                else None
            )
            last_scan_started_at = scan_started_at
            if ui is not None:
                action = ui.render(snapshot)
                if action is None and hasattr(ui, "consume_action"):
                    action = ui.consume_action()
                if action and hasattr(scanner, "perform_action"):
                    scanner.perform_action(action)
                    snapshot = _snapshot_dict(scanner.scan_once())
                    snapshot["target_interval_seconds"] = sleep_seconds
                    snapshot["actual_cycle_ms"] = (
                        int((scan_started_at - last_scan_started_at) * 1000)
                        if last_scan_started_at is not None
                        else None
                    )
                    ui.render(snapshot)

            metrics = _metrics_payload(snapshot)
            write_metrics_snapshot(root / "metrics.json", metrics)
            write_dashboard_bundle(root, snapshot)
            write_alert_snapshot(root / "alerts.json", _alerts(snapshot))
            write_health_snapshot(
                root / "health.json",
                HealthStatus(
                    feed_fresh=_read(snapshot, "source_status", _read(snapshot, "status", "unknown")) == "ok",
                    archive_ok=True,
                    parse_error_rate_ok=_read(snapshot, "source_status", _read(snapshot, "status", "unknown")) != "parse_error",
                ),
            )

            completed += 1
            if sleep_seconds and (iterations <= 0 or completed < iterations):
                sleep_fn(sleep_seconds)
    finally:
        if ui is not None:
            ui.close()

    return completed


def run_monitor_iteration(root: str | Path, scanner, limit: int = 50):
    if hasattr(scanner, "scan_once"):
        original_snapshot = scanner.scan_once()
    else:
        original_snapshot = scanner.scan(limit=limit)
    snapshot = _snapshot_dict(original_snapshot)
    metrics = _metrics_payload(snapshot)
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    write_metrics_snapshot(root / "metrics.json", metrics)
    write_dashboard_bundle(root, snapshot)
    write_alert_snapshot(root / "alerts.json", _alerts(snapshot))
    write_health_snapshot(
        root / "health.json",
        HealthStatus(
            feed_fresh=_read(snapshot, "source_status", _read(snapshot, "status", "unknown")) == "ok",
            archive_ok=True,
            parse_error_rate_ok=_read(snapshot, "source_status", _read(snapshot, "status", "unknown")) != "parse_error",
        ),
    )
    return original_snapshot
