import tempfile
import unittest
import json
from pathlib import Path

from polymarket_arb.app.monitor_cli import main
from polymarket_arb.auth.manual_order import ManualOrderConfig, ManualOrderResult


class MonitorCliTests(unittest.TestCase):
    def test_cli_runs_once(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code = main(["--root", str(root), "--once", "--limit", "5", "--demo", "--no-ui"])
            self.assertEqual(code, 0)
            self.assertTrue((root / "metrics.json").exists())

    def test_demo_scanner_exposes_recommended_orders(self) -> None:
        from polymarket_arb.app.monitor_cli import _DemoScanner

        snapshot = _DemoScanner().scan_once()

        self.assertTrue(snapshot["candidates"])
        self.assertTrue(snapshot["candidates"][0]["recommended_orders"])
        self.assertEqual(snapshot["event_count"], 50)
        self.assertEqual(snapshot["scan_limit"], 50)
        self.assertEqual(snapshot["page_count"], 1)

    def test_cli_passes_event_limit_into_default_gamma_scanner(self) -> None:
        from polymarket_arb.monitoring.models import MonitorSnapshot

        class FakeGammaClient:
            pass

        calls = {}

        class FakeGammaScanner:
            def __init__(self, client, min_edge_threshold=0.03, limit=500):
                calls["min_edge_threshold"] = min_edge_threshold
                calls["limit"] = limit

            def scan_once(self):
                return MonitorSnapshot(
                    iteration=1,
                    event_count=calls["limit"],
                    complete_event_count=0,
                    candidate_count=0,
                    best_gross_edge=0.0,
                    best_cost_adjusted_edge=0.0,
                    scan_duration_seconds=0.1,
                    candidates=[],
                    rejections=[],
                )

        import polymarket_arb.app.monitor_cli as monitor_cli

        original_client = monitor_cli.GammaMarketClient
        original_scanner = monitor_cli.GammaScanner
        monitor_cli.GammaMarketClient = FakeGammaClient
        monitor_cli.GammaScanner = FakeGammaScanner
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                code = main(["--root", str(root), "--once", "--limit", "250", "--min-edge", "0.02", "--no-ui"])
                self.assertEqual(code, 0)
                self.assertEqual(calls["limit"], 250)
                self.assertEqual(calls["min_edge_threshold"], 0.02)
        finally:
            monitor_cli.GammaMarketClient = original_client
            monitor_cli.GammaScanner = original_scanner

    def test_cli_default_candidate_threshold_is_one_percent(self) -> None:
        from polymarket_arb.monitoring.models import MonitorSnapshot

        calls = {}

        class FakeGammaClient:
            pass

        class FakeGammaScanner:
            def __init__(self, client, min_edge_threshold=0.03, limit=500):
                calls["min_edge_threshold"] = min_edge_threshold
                calls["limit"] = limit

            def scan_once(self):
                return MonitorSnapshot(
                    iteration=1,
                    event_count=0,
                    complete_event_count=0,
                    candidate_count=0,
                    best_gross_edge=0.0,
                    best_cost_adjusted_edge=0.0,
                    scan_duration_seconds=0.1,
                    candidates=[],
                    rejections=[],
                )

        import polymarket_arb.app.monitor_cli as monitor_cli

        original_client = monitor_cli.GammaMarketClient
        original_scanner = monitor_cli.GammaScanner
        monitor_cli.GammaMarketClient = FakeGammaClient
        monitor_cli.GammaScanner = FakeGammaScanner
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                code = main(["--root", str(root), "--once", "--no-ui"])
                self.assertEqual(code, 0)
                self.assertEqual(calls["min_edge_threshold"], 0.01)
        finally:
            monitor_cli.GammaMarketClient = original_client
            monitor_cli.GammaScanner = original_scanner

    def test_cli_uses_clob_snapshot_loader_when_available(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            def loader(_root):
                return (
                    {
                        "mode": "account-live",
                        "creds_present": True,
                        "sdk_available": True,
                        "reason": "clob account api connected",
                    },
                    {
                        "balances": {"balance": "12.34"},
                        "pnl_summary": {"estimated_total_pnl": "1.23", "open_position_count": 1},
                        "positions": [{"title": "Election A", "outcome": "YES", "net_quantity": 2, "mark_price": 0.6, "estimated_pnl": 0.2}],
                        "recent_trades": [{"title": "Election A", "outcome": "YES", "side": "BUY", "size": "2", "price": "0.5"}],
                    },
                )

            code = main(
                ["--root", str(root), "--once", "--limit", "5", "--demo", "--no-ui"],
                clob_snapshot_loader=loader,
            )
            self.assertEqual(code, 0)
            payload = json.loads((root / "metrics.json").read_text())
            self.assertEqual(payload["account_status"]["mode"], "account-live")
            self.assertEqual(payload["account_snapshot"]["balances"]["balance"], "12.34")

    def test_cli_account_only_mode_sets_account_view(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            def loader(_root):
                return (
                    {
                        "mode": "account-ready",
                        "creds_present": True,
                        "sdk_available": True,
                        "reason": "missing private key for clob level2 auth",
                    },
                    None,
                )

            code = main(
                ["--root", str(root), "--once", "--account-only", "--no-ui"],
                clob_snapshot_loader=loader,
            )
            self.assertEqual(code, 0)
            payload = json.loads((root / "metrics.json").read_text())
            self.assertEqual(payload["view"], "account")
            self.assertEqual(payload["account_status"]["mode"], "account-ready")

    def test_cli_surfaces_live_error_detail_in_reason(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            def loader(_root):
                return (
                    {
                        "mode": "account-auth-ready",
                        "creds_present": True,
                        "sdk_available": True,
                        "reason": "clob account api failed: PolyApiException[status_code=401, error_message={'error': 'Unauthorized/Invalid api key'}]",
                    },
                    None,
                )

            code = main(
                ["--root", str(root), "--once", "--account-only", "--no-ui"],
                clob_snapshot_loader=loader,
            )
            self.assertEqual(code, 0)
            payload = json.loads((root / "metrics.json").read_text())
            self.assertIn("Unauthorized/Invalid api key", payload["account_status"]["reason"])

    def test_cli_submit_action_refreshes_snapshot_synchronously(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            class FakeUI:
                def __init__(self):
                    self.calls = 0

                def render(self, snapshot):
                    self.calls += 1
                    if self.calls == 1:
                        return {"type": "submit_order"}
                    return None

                def close(self):
                    return None

            state = {"submitted": False}

            def loader(_root):
                return (
                    {
                        "mode": "account-live",
                        "creds_present": True,
                        "sdk_available": True,
                        "reason": "clob account api connected",
                    },
                    {
                        "balances": {"balance": "12.34"},
                        "pnl_summary": {"estimated_total_pnl": "1.23", "open_position_count": 1},
                        "positions": [{"title": "Election A", "outcome": "YES", "net_quantity": 2, "mark_price": 0.6, "estimated_pnl": 0.2}],
                        "recent_trades": [{"title": "Election A", "outcome": "YES", "side": "BUY", "size": "2", "price": "0.5"}],
                    },
                )

            def draft_loader(_root):
                return ManualOrderConfig(token_id="token-1", side="BUY", price=0.42, size=5.0, order_type="GTC")

            def submitter(draft, root="."):
                state["submitted"] = True
                return ManualOrderResult(success=True, reason="submitted", order_id="ord-123")

            code = main(
                ["--root", str(root), "--once", "--account-only"],
                ui_factory=FakeUI,
                clob_snapshot_loader=loader,
                manual_order_loader=draft_loader,
                manual_order_submitter=submitter,
            )
            self.assertEqual(code, 0)
            self.assertTrue(state["submitted"])

    def test_real_ui_pending_action_is_consumed(self) -> None:
        class FakeUI:
            def __init__(self):
                self.pending_action = {"type": "submit_order"}

            def render(self, snapshot):
                return None

            def consume_action(self):
                action = self.pending_action
                self.pending_action = None
                return action

            def close(self):
                return None

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            state = {"submitted": False}

            def loader(_root):
                return (
                    {"mode": "account-live", "creds_present": True, "sdk_available": True, "reason": "clob account api connected"},
                    {"balances": {"balance": "1"}, "pnl_summary": {"estimated_total_pnl": "0", "open_position_count": 0}, "positions": [], "recent_trades": []},
                )

            def submitter(draft, root="."):
                state["submitted"] = True
                return ManualOrderResult(success=True, reason="submitted", order_id="ord-123")

            code = main(
                ["--root", str(root), "--once"],
                ui_factory=FakeUI,
                clob_snapshot_loader=loader,
                manual_order_loader=lambda _root: ManualOrderConfig(token_id="token-1", side="BUY", price=0.42, size=5.0, order_type="GTC"),
                manual_order_submitter=submitter,
            )
            self.assertEqual(code, 0)
            self.assertTrue(state["submitted"])

    def test_cli_uses_rule_recommendation_when_manual_draft_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            state = {"submitted": False}

            class FakeUI:
                def __init__(self):
                    self.calls = 0

                def render(self, snapshot):
                    self.calls += 1
                    if self.calls == 1:
                        return {"type": "submit_order"}
                    return None

                def close(self):
                    return None

            def loader(_root):
                return (
                    {"mode": "account-live", "creds_present": True, "sdk_available": True, "reason": "clob account api connected"},
                    {"balances": {"balance": "1"}, "pnl_summary": {"estimated_total_pnl": "0", "open_position_count": 0}, "positions": [], "recent_trades": []},
                )

            scanner = type(
                "StubScanner",
                (),
                {
                    "scan_once": lambda self: {
                        "candidates": [
                            {
                                "title": "Best Event",
                                "template_type": "market_cap_buckets",
                                "category": "Politics",
                                "gross_edge": 0.08,
                                "adjusted_edge": 0.05,
                                "recommended_orders": [
                                    {"token_id": "t1", "side": "BUY", "price": 0.45, "size": 5.0, "order_min_size": 5.0},
                                    {"token_id": "t2", "side": "BUY", "price": 0.47, "size": 5.0, "order_min_size": 5.0},
                                ],
                            }
                        ]
                    },
                },
            )()

            def submitter(draft, root="."):
                state["submitted"] = draft is not None and isinstance(draft, dict) and len(draft.get("legs", [])) == 2
                return ManualOrderResult(success=True, reason="submitted", order_id="ord-bundle")

            code = main(
                ["--root", str(root), "--once"],
                scanner=scanner,
                ui_factory=FakeUI,
                clob_snapshot_loader=loader,
                manual_order_loader=lambda _root: None,
                manual_order_submitter=submitter,
            )
            self.assertEqual(code, 0)
            self.assertTrue(state["submitted"])

    def test_cli_tracks_account_pnl_deltas_across_iterations(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = {"count": 0}

            def loader(_root):
                calls["count"] += 1
                if calls["count"] == 1:
                    return (
                        {"mode": "account-live", "creds_present": True, "sdk_available": True, "reason": "clob account api connected"},
                        {
                            "balances": {"balance": "12.34"},
                            "pnl_summary": {
                                "estimated_total_pnl": 1.00,
                                "estimated_equity": 4.00,
                                "fees_paid": 0.10,
                                "open_position_count": 1,
                            },
                            "positions": [
                                {
                                    "asset_id": "tok-a",
                                    "title": "Election A",
                                    "outcome": "YES",
                                    "net_quantity": 2,
                                    "mark_price": 0.6,
                                    "estimated_pnl": 0.50,
                                }
                            ],
                            "recent_trades": [],
                        },
                    )
                return (
                    {"mode": "account-live", "creds_present": True, "sdk_available": True, "reason": "clob account api connected"},
                    {
                        "balances": {"balance": "12.84"},
                        "pnl_summary": {
                            "estimated_total_pnl": 1.50,
                            "estimated_equity": 4.40,
                            "fees_paid": 0.12,
                            "open_position_count": 1,
                        },
                        "positions": [
                            {
                                "asset_id": "tok-a",
                                "title": "Election A",
                                "outcome": "YES",
                                "net_quantity": 2,
                                "mark_price": 0.62,
                                "estimated_pnl": 0.75,
                            }
                        ],
                        "recent_trades": [],
                    },
                )

            code = main(
                ["--root", str(root), "--iterations", "2", "--sleep-seconds", "0", "--limit", "5", "--demo", "--no-ui"],
                clob_snapshot_loader=loader,
            )
            self.assertEqual(code, 0)
            payload = json.loads((root / "metrics.json").read_text())
            pnl = payload["account_snapshot"]["pnl_summary"]
            self.assertAlmostEqual(pnl["estimated_total_pnl_delta"], 0.5)
            self.assertAlmostEqual(pnl["estimated_equity_delta"], 0.4)
            self.assertAlmostEqual(pnl["fees_paid_delta"], 0.02)
            self.assertAlmostEqual(payload["account_snapshot"]["positions"][0]["estimated_pnl_delta"], 0.25)
            self.assertEqual(payload["target_interval_seconds"], 0.0)
            self.assertGreaterEqual(payload["actual_cycle_ms"], 0)

    def test_cli_carries_last_full_rule_combo_into_hot_snapshot_reference(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            class StubScanner:
                def __init__(self):
                    self.calls = 0

                def scan_once(self):
                    self.calls += 1
                    if self.calls == 1:
                        return {
                            "scan_mode": "full",
                            "candidates": [
                                {
                                    "title": "Best Event",
                                    "template_type": "market_cap_buckets",
                                    "category": "Politics",
                                    "gross_edge": 0.08,
                                    "adjusted_edge": 0.05,
                                    "recommended_orders": [
                                        {"token_id": "t1", "side": "BUY", "price": 0.45, "size": 5.0, "order_min_size": 5.0},
                                        {"token_id": "t2", "side": "BUY", "price": 0.47, "size": 5.0, "order_min_size": 5.0},
                                    ],
                                }
                            ],
                        }
                    return {
                        "scan_mode": "hot",
                        "reference_candidate_count": 1,
                        "reference_best_gross_edge": 0.04,
                        "candidates": [],
                    }

            code = main(
                ["--root", str(root), "--iterations", "2", "--sleep-seconds", "0", "--no-ui"],
                scanner=StubScanner(),
                clob_snapshot_loader=lambda _root: ({"mode": "account-live", "creds_present": True, "sdk_available": True, "reason": "clob account api connected"}, {"balances": {"balance": "1"}, "positions": [], "recent_trades": [], "pnl_summary": {"estimated_total_pnl": 0, "estimated_equity": 0, "fees_paid": 0, "open_position_count": 0}}),
                manual_order_loader=lambda _root: None,
            )
            self.assertEqual(code, 0)
            payload = json.loads((root / "metrics.json").read_text())
            self.assertIsNone(payload.get("order_draft"))
            self.assertEqual(payload["reference_order_draft"]["title"], "Best Event")
            self.assertEqual(len(payload["reference_order_draft"]["legs"]), 2)
