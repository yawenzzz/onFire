from __future__ import annotations

import argparse
import os
import sys
from dataclasses import dataclass
from pathlib import Path

from polymarket_arb.app.monitor_tui import CursesMonitorView
from polymarket_arb.auth.account_status import _read_env_file, detect_account_status, load_pm_auth_from_root
from polymarket_arb.auth.manual_order import load_manual_order_config, submit_manual_order
from polymarket_arb.monitoring.account_client import AccountClient
from polymarket_arb.monitoring.clob_account_service import load_clob_account_snapshot
from polymarket_arb.monitoring.gamma_client import GammaMarketClient
from polymarket_arb.monitoring.order_recommendation import recommend_order_draft
from polymarket_arb.monitoring.scanner import RealtimeGammaScanner

GammaScanner = RealtimeGammaScanner
from polymarket_arb.service.monitor_daemon import run_monitor_daemon


def _num(value, default: float = 0.0) -> float:
    if value is None:
        return default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _position_key(position) -> tuple[str, str, str, str]:
    if not isinstance(position, dict):
        position = getattr(position, "__dict__", {}) or {}
    return (
        str(position.get("asset_id", "")),
        str(position.get("title", "")),
        str(position.get("outcome", "")),
        str(position.get("market", "")),
    )


def _attach_account_pnl_deltas(account_snapshot, previous_snapshot):
    if not isinstance(account_snapshot, dict):
        return account_snapshot

    previous_snapshot = previous_snapshot if isinstance(previous_snapshot, dict) else {}
    enriched = dict(account_snapshot)
    previous_summary = previous_snapshot.get("pnl_summary", {}) if isinstance(previous_snapshot, dict) else {}
    current_summary = dict(enriched.get("pnl_summary", {}) or {})
    if current_summary:
        current_summary["estimated_total_pnl_delta"] = round(
            _num(current_summary.get("estimated_total_pnl")) - _num(previous_summary.get("estimated_total_pnl")),
            6,
        )
        current_summary["estimated_equity_delta"] = round(
            _num(current_summary.get("estimated_equity")) - _num(previous_summary.get("estimated_equity")),
            6,
        )
        current_summary["fees_paid_delta"] = round(
            _num(current_summary.get("fees_paid")) - _num(previous_summary.get("fees_paid")),
            6,
        )
        enriched["pnl_summary"] = current_summary

    previous_positions = previous_snapshot.get("positions", []) if isinstance(previous_snapshot, dict) else []
    previous_position_map = {
        _position_key(item): item
        for item in previous_positions
        if isinstance(item, dict) or getattr(item, "__dict__", None)
    }
    current_positions = []
    for item in enriched.get("positions", []) or []:
        position = dict(item) if isinstance(item, dict) else dict(getattr(item, "__dict__", {}) or {})
        previous_position = previous_position_map.get(_position_key(position), {})
        position["estimated_pnl_delta"] = round(
            _num(position.get("estimated_pnl")) - _num(previous_position.get("estimated_pnl")),
            6,
        )
        position["estimated_equity_delta"] = round(
            _num(position.get("estimated_equity")) - _num(previous_position.get("estimated_equity")),
            6,
        )
        current_positions.append(position)
    if current_positions:
        enriched["positions"] = current_positions
    return enriched


@dataclass
class _DemoScanner:
    calls: int = 0

    def scan_once(self) -> dict:
        self.calls += 1
        gross_edge = max(0.0, 0.05 - (self.calls * 0.005))
        adjusted_edge = gross_edge - 0.03
        return {
            "updated_at": f"demo-{self.calls}",
            "scan_latency_ms": 1200,
            "source_status": "ok",
            "event_count": 50,
            "complete_event_count": 1 if gross_edge > 0 else 0,
            "scan_limit": 50,
            "page_count": 1,
            "candidate_count": 1 if gross_edge > 0 else 0,
            "rejection_count": 1,
            "best_gross_edge": gross_edge,
            "best_adjusted_edge": adjusted_edge,
            "candidates": [
                {
                    "title": "Demo complete bucket",
                    "gross_edge": gross_edge,
                    "adjusted_edge": adjusted_edge,
                    "reason": "complete_bucket",
                    "recommended_orders": [
                        {
                            "token_id": "demo-token-1",
                            "title": "Demo yes leg",
                            "side": "BUY",
                            "price": 0.45,
                            "size": 5.0,
                            "order_type": "GTC",
                        },
                        {
                            "token_id": "demo-token-2",
                            "title": "Demo no leg",
                            "side": "BUY",
                            "price": 0.50,
                            "size": 5.0,
                            "order_type": "GTC",
                        },
                    ],
                }
            ]
            if gross_edge > 0
            else [],
            "rejections": [{"title": "Demo reject", "reason": "overlap_detected"}],
            "rejection_reason_counts": {"overlap_detected": 1},
        }


@dataclass
class _AccountOnlyScanner:
    calls: int = 0

    def scan_once(self) -> dict:
        self.calls += 1
        return {
            "view": "account",
            "updated_at": f"account-{self.calls}",
            "scan_latency_ms": 50,
            "source_status": "ok",
            "data_source": "clob-account",
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [],
            "rejections": [],
            "rejection_reason_counts": {},
            "watched_events": [],
        }


def main(
    argv: list[str] | None = None,
    scanner=None,
    ui_factory=None,
    clob_snapshot_loader=None,
    manual_order_loader=None,
    manual_order_submitter=None,
) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--iterations", type=int, default=0)
    parser.add_argument("--once", action="store_true")
    parser.add_argument("--sleep-seconds", type=float, default=3.0)
    parser.add_argument("--scan-interval-seconds", type=float)
    parser.add_argument("--event-limit", "--limit", dest="event_limit", type=int, default=500)
    parser.add_argument("--min-edge", type=float, default=0.01)
    parser.add_argument("--no-ui", action="store_true")
    parser.add_argument("--demo", action="store_true")
    parser.add_argument("--account-only", action="store_true")
    args = parser.parse_args(argv)
    root_env = {}
    root_env.update(_read_env_file(Path(args.root) / ".env"))
    root_env.update(_read_env_file(Path(args.root) / ".env.local"))
    for key, value in root_env.items():
        os.environ.setdefault(key, value)
    generated_pm_auth = load_pm_auth_from_root(args.root)
    if generated_pm_auth is not None:
        os.environ["PM_ACCESS_KEY"] = generated_pm_auth["access_key"]
        os.environ["PM_SIGNATURE"] = generated_pm_auth["signature"]
        os.environ["PM_TIMESTAMP"] = generated_pm_auth["timestamp"]
    sleep_seconds = args.scan_interval_seconds if args.scan_interval_seconds is not None else args.sleep_seconds
    iterations = 1 if args.once else args.iterations

    if scanner is None:
        if args.account_only:
            scanner = _AccountOnlyScanner()
        elif args.demo:
            scanner = _DemoScanner()
        else:
            scanner = GammaScanner(
                client=GammaMarketClient(),
                min_edge_threshold=args.min_edge,
                limit=args.event_limit,
            )

    if hasattr(scanner, "scan") and not hasattr(scanner, "scan_once"):
        event_limit = args.event_limit

        class _ScannerAdapter:
            def scan_once(self_nonlocal):
                return scanner.scan(limit=event_limit)

        scanner = _ScannerAdapter()

    ui = None
    if not args.no_ui and (sys.stdout.isatty() or ui_factory is not None):
        factory = ui_factory or CursesMonitorView
        ui = factory()

    if scanner is not None and not hasattr(scanner, "account_status"):
        account_status = detect_account_status(args.root)
        pm_auth = load_pm_auth_from_root(args.root)
        account_client = AccountClient() if pm_auth else None
        clob_snapshot_loader = clob_snapshot_loader or load_clob_account_snapshot
        manual_order_loader = manual_order_loader or load_manual_order_config
        manual_order_submitter = manual_order_submitter or submit_manual_order

        if hasattr(scanner, "scan_once"):
            inner_scanner = scanner

            class _AccountAwareScanner:
                def __init__(self) -> None:
                    self.last_order_result = None
                    self.last_account_snapshot = None
                    self.last_full_rule_draft = None

                @staticmethod
                def _serializable(value):
                    if value is None or isinstance(value, (str, int, float, bool, list, dict)):
                        return value
                    return getattr(value, "__dict__", value)

                def scan_once(self_nonlocal):
                    snapshot = inner_scanner.scan_once()
                    account_snapshot = None
                    account_status_local, account_snapshot = clob_snapshot_loader(args.root)
                    order_draft = manual_order_loader(args.root)
                    snapshot_dict = snapshot if isinstance(snapshot, dict) else getattr(snapshot, "__dict__", {}) or {}
                    if order_draft is None and snapshot_dict:
                        order_draft = recommend_order_draft(snapshot_dict)
                    reference_order_draft = None
                    if snapshot_dict:
                        if snapshot_dict.get("scan_mode") == "full" and isinstance(order_draft, dict) and order_draft.get("source") == "rule":
                            self_nonlocal.last_full_rule_draft = order_draft
                        elif snapshot_dict.get("scan_mode") == "hot":
                            reference_order_draft = self_nonlocal.last_full_rule_draft
                    if account_snapshot is None:
                        account_status_local = dict(account_status if not account_status_local else account_status_local)
                    if account_snapshot is None and account_client is not None:
                        try:
                            balances = account_client.fetch_account_balances(pm_auth)
                            open_orders = account_client.fetch_open_orders(pm_auth)
                            activities = account_client.fetch_activities(pm_auth, limit=5)
                            account_snapshot = {
                                "balances": balances,
                                "open_orders": open_orders,
                                "activities": activities,
                            }
                            account_status_local = dict(account_status)
                            account_status_local["mode"] = "account-live"
                            account_status_local["reason"] = "private account api connected"
                        except Exception as exc:
                            account_status_local["reason"] = f"private account api failed: {type(exc).__name__}"
                    if account_snapshot is not None:
                        account_snapshot = _attach_account_pnl_deltas(account_snapshot, self_nonlocal.last_account_snapshot)
                        self_nonlocal.last_account_snapshot = account_snapshot
                    if isinstance(snapshot, dict):
                        snapshot["account_status"] = account_status_local
                        snapshot["account_snapshot"] = account_snapshot
                        snapshot["order_draft"] = self_nonlocal._serializable(order_draft)
                        snapshot["reference_order_draft"] = self_nonlocal._serializable(reference_order_draft)
                        snapshot["order_result"] = self_nonlocal._serializable(self_nonlocal.last_order_result)
                    else:
                        snapshot.account_status = account_status_local
                        snapshot.account_snapshot = account_snapshot
                        snapshot.order_draft = self_nonlocal._serializable(order_draft)
                        snapshot.reference_order_draft = self_nonlocal._serializable(reference_order_draft)
                        snapshot.order_result = self_nonlocal._serializable(self_nonlocal.last_order_result)
                    return snapshot

                def perform_action(self_nonlocal, action: dict) -> None:
                    if action.get("type") != "submit_order":
                        return
                    draft = manual_order_loader(args.root)
                    if draft is None:
                        current_snapshot = self_nonlocal.scan_once()
                        if isinstance(current_snapshot, dict):
                            draft = current_snapshot.get("order_draft") or recommend_order_draft(current_snapshot)
                    self_nonlocal.last_order_result = manual_order_submitter(draft, root=args.root)

            scanner = _AccountAwareScanner()

    run_monitor_daemon(
        root=args.root,
        scanner=scanner,
        iterations=iterations,
        sleep_seconds=sleep_seconds,
        ui=ui,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
