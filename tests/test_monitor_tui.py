import unittest

from polymarket_arb.monitoring.tui import render_snapshot_lines
from polymarket_arb.monitoring.models import EventScanResult, MonitorSnapshot
from polymarket_arb.app.monitor_tui import _handle_account_key, render_snapshot_lines as app_render_snapshot_lines
from polymarket_arb.auth.manual_order import ManualOrderConfig, ManualOrderResult


class MonitorTuiTests(unittest.TestCase):
    def test_render_snapshot_lines_includes_key_monitoring_sections(self) -> None:
        candidate = EventScanResult(
            slug="fannie-mae-ipo-closing-market-cap",
            title="Fannie Mae IPO Closing Market Cap",
            template_type="complete_bucket",
            is_complete=True,
            open_market_count=7,
            total_market_count=7,
            sum_ask=0.999,
            gross_edge=0.001,
            cost_adjusted_edge=-0.029,
            rejection_reason=None,
            questions=[],
        )
        snapshot = MonitorSnapshot(
            iteration=3,
            event_count=20,
            complete_event_count=1,
            candidate_count=1,
            best_gross_edge=0.001,
            best_cost_adjusted_edge=-0.029,
            scan_duration_seconds=1.2,
            candidates=[candidate],
            rejections=[],
            rejection_counts={"unsupported_structure": 19},
            data_source="gamma-api",
            status="ok",
        )

        lines = render_snapshot_lines(snapshot, width=100, height=24)
        text = "\n".join(lines)

        self.assertIn("Polymarket Monitor", text)
        self.assertIn("[OK]", text)
        self.assertIn("OVERVIEW", text)
        self.assertIn("ACCOUNT", text)
        self.assertIn("CANDIDATE QUEUE", text)
        self.assertIn("RULES / THRESHOLDS", text)
        self.assertIn("Fannie Mae IPO Closing Market Cap", text)

    def test_render_snapshot_lines_shows_structured_summary_rows(self) -> None:
        snapshot = MonitorSnapshot(
            iteration=7,
            event_count=50,
            complete_event_count=8,
            candidate_count=2,
            best_gross_edge=0.034,
            best_cost_adjusted_edge=0.004,
            scan_duration_seconds=1.8,
            candidates=[],
            rejections=[],
            rejection_counts={"unsupported_structure": 40, "open_candidate_set": 8},
            data_source="gamma-api",
            status="ok",
            updated_at="2026-04-08T07:20:00+00:00",
            scan_mode="hot",
            hot_limit=100,
            full_scan_every=5,
            scan_offset=200,
            reference_candidate_count=1,
            reference_best_gross_edge=0.027,
        )

        lines = render_snapshot_lines(snapshot, width=100, height=16)
        text = "\n".join(lines)

        self.assertIn("iteration=7", text.lower())
        self.assertIn("scan latency", text.lower())
        self.assertIn("candidate queue", text.lower())
        self.assertIn("FUNNEL", text)
        self.assertIn("RULES", text)
        self.assertIn("mode=HOT", text)
        self.assertIn("every=5", text)
        self.assertIn("window=200:250", text)
        self.assertIn("refFull=1@+0.027", text)

    def test_render_snapshot_lines_uses_split_layout_on_wide_terminal(self) -> None:
        candidate = EventScanResult(
            slug="discord-ipo",
            title="Discord IPO Closing Market Cap",
            template_type="market_cap_buckets",
            is_complete=True,
            open_market_count=6,
            total_market_count=6,
            sum_ask=0.96,
            gross_edge=0.04,
            cost_adjusted_edge=0.01,
            rejection_reason=None,
            questions=[],
        )
        snapshot = MonitorSnapshot(
            iteration=2,
            event_count=50,
            complete_event_count=4,
            candidate_count=1,
            best_gross_edge=0.04,
            best_cost_adjusted_edge=0.01,
            scan_duration_seconds=1.3,
            candidates=[candidate],
            rejections=[],
            rejection_counts={"unsupported_structure": 44, "open_candidate_set": 5},
            data_source="gamma-api",
            status="ok",
            updated_at="2026-04-08T07:30:00+00:00",
        )

        lines = render_snapshot_lines(snapshot, width=120, height=22)
        text = "\n".join(lines)

        self.assertIn("CANDIDATE QUEUE", text)
        self.assertIn("ACCOUNT", text)
        self.assertIn("RULES / THRESHOLDS", text)
        self.assertIn("FOCUSED CANDIDATE", text)
        self.assertIn("│", text)
        self.assertIn("╭", text)
        self.assertIn("╮", text)
        self.assertIn("controls:", text.lower())

    def test_render_snapshot_lines_shows_watched_events_and_categories(self) -> None:
        snapshot = {
            "iteration": 1,
            "updated_at": "2026-04-08T07:40:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "scan_latency_ms": 2100,
            "event_count": 10,
            "complete_event_count": 1,
            "candidate_count": 0,
            "rejection_count": 10,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [],
            "rejection_reason_counts": {"unsupported_structure": 8, "open_candidate_set": 2},
            "category_counts": {"Politics": 6, "Crypto": 4},
            "watched_events": [
                {"title": "Starmer out by...?", "category": "Politics", "status": "REJECT", "reason": "unsupported_structure", "structure": "unsupported_structure"},
                {"title": "Pump.fun airdrop by ....? ", "category": "Crypto", "status": "REJECT", "reason": "unsupported_structure", "structure": "unsupported_structure"},
            ],
        }

        lines = render_snapshot_lines(snapshot, width=120, height=24)
        text = "\n".join(lines)

        self.assertIn("CANDIDATE QUEUE (0)", text)
        self.assertIn("No candidates", text)
        self.assertIn("FILTER CHAIN", text)

    def test_render_snapshot_lines_uses_top_rejected_when_no_candidates(self) -> None:
        snapshot = {
            "iteration": 4,
            "updated_at": "2026-04-08T09:00:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "scan_latency_ms": 1800,
            "event_count": 50,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 50,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [],
            "rejections": [
                {
                    "title": "Discord IPO Closing Market Cap",
                    "gross_edge": 0.012,
                    "adjusted_edge": -0.018,
                    "reason": "edge_below_threshold",
                    "category": "Tech",
                },
                {
                    "title": "Fannie Mae IPO Closing Market Cap",
                    "gross_edge": 0.004,
                    "adjusted_edge": -0.026,
                    "reason": "edge_below_threshold",
                    "category": "Finance",
                },
            ],
            "rejection_reason_counts": {"edge_below_threshold": 2, "unsupported_structure": 48},
            "category_counts": {"Tech": 1, "Finance": 1},
            "structure_counts": {"market_cap_buckets": 2, "unsupported_structure": 48},
            "watched_events": [],
        }

        lines = render_snapshot_lines(snapshot, width=120, height=20)
        text = "\n".join(lines)

        self.assertIn("CANDIDATE QUEUE (0)", text)
        self.assertIn("No candidates", text)

    def test_render_snapshot_lines_shows_candidate_queue_headers_and_zero_state(self) -> None:
        snapshot = {
            "iteration": 2,
            "updated_at": "2026-04-08T07:45:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "scan_latency_ms": 1900,
            "event_count": 50,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 50,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [],
            "rejection_reason_counts": {"unsupported_structure": 44},
            "category_counts": {"Politics": 26},
            "structure_counts": {"unsupported_structure": 44, "open_candidate_set": 5},
            "watched_events": [],
        }

        lines = render_snapshot_lines(snapshot, width=120, height=20)
        text = "\n".join(lines)

        self.assertIn("CANDIDATE QUEUE", text)
        self.assertIn("gross", text.lower())
        self.assertIn("net", text.lower())
        self.assertIn("No candidates", text)

    def test_render_snapshot_lines_shows_market_metrics_and_focused_event(self) -> None:
        snapshot = {
            "iteration": 5,
            "updated_at": "2026-04-08T09:10:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "signer_address": "0x11084005d88a0840b5f38f8731cca9152bbd99f7",
                "funder_address": "0x97c59bce45c910f80cae2a84ee2c8ac0d0069c9f",
                "reason": "private account api connected",
            },
            "account_snapshot": {
                "balances": {
                    "currentBalance": {"value": "123.45"},
                    "buyingPower": {"value": "67.89"},
                    "openOrders": {"value": "10.00"},
                    "unsettledFunds": {"value": "1.23"},
                },
                "open_orders": [
                    {
                        "title": "Starmer out by...?",
                        "outcome": "YES",
                        "side": "BUY",
                        "price": "0.61",
                        "original_size": "3",
                        "status": "OPEN",
                    }
                ],
                "pnl_summary": {
                    "estimated_total_pnl": 12.34,
                    "estimated_equity": 45.67,
                    "fees_paid": 0.89,
                    "open_position_count": 2,
                    "estimated_total_pnl_delta": 0.56,
                    "estimated_equity_delta": 0.12,
                },
                "positions": [
                    {
                        "title": "Starmer out by...?",
                        "outcome": "YES",
                        "net_quantity": 6.0,
                        "mark_price": 0.62,
                        "estimated_pnl": 1.50,
                        "estimated_pnl_delta": 0.21,
                    },
                    {
                        "title": "Pump.fun airdrop by...?",
                        "outcome": "NO",
                        "net_quantity": 5.0,
                        "mark_price": 0.40,
                        "estimated_pnl": 0.49,
                        "estimated_pnl_delta": -0.07,
                    },
                ],
                "recent_trades": [
                    {
                        "title": "Starmer out by...?",
                        "outcome": "YES",
                        "side": "SELL",
                        "size": "4",
                        "price": "0.70",
                    }
                ],
            },
            "order_draft": {
                "source": "rule",
                "title": "Best Combo",
                "legs": [
                    {"token_id": "t1", "side": "BUY", "price": 0.61, "size": 3.0},
                    {"token_id": "t2", "side": "BUY", "price": 0.24, "size": 3.0},
                ],
                "order_type": "GTC",
                "bundle_size": 3.0,
                "total_price": 0.85,
                "net_edge": 0.015,
            },
            "scan_latency_ms": 1800,
            "event_count": 10,
            "complete_event_count": 0,
            "candidate_count": 1,
            "rejection_count": 10,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [
                {
                    "title": "Starmer out by...?",
                    "gross_edge": 0.02,
                    "adjusted_edge": -0.01,
                    "template_type": "market_cap_buckets",
                    "category": "Politics",
                    "best_bid": 0.26,
                    "best_ask": 0.27,
                    "last_trade_price": 0.265,
                    "volume_24hr": 14961.0,
                    "liquidity": 146438.1,
                    "depth_bid_top": 2000.0,
                    "depth_ask_top": 1500.0,
                    "repeat_interval_ms": 842,
                    "event_url": "https://polymarket.com/event/starmer-out-in-2025",
                    "recommended_orders": [
                        {"token_id": "t1", "side": "BUY", "price": 0.61, "size": 3.0}
                    ],
                }
            ],
            "rejections": [],
            "rejection_reason_counts": {"unsupported_structure": 8},
            "category_counts": {"Politics": 6},
            "structure_counts": {"unsupported_structure": 8},
            "watched_events": [],
        }

        lines = render_snapshot_lines(snapshot, width=150, height=32)
        text = "\n".join(lines)

        self.assertIn("bid", text.lower())
        self.assertIn("ask", text.lower())
        self.assertIn("vol24", text.lower())
        self.assertIn("dBid", text)
        self.assertIn("FOCUSED CANDIDATE", text)
        self.assertIn("starmer-out-in-2025", text)
        self.assertIn("balance=123.45", text)
        self.assertIn("buyingPower=67.89", text)
        self.assertIn("account-live", text)
        self.assertIn("sigType=2", text)
        self.assertIn("funder=yes", text)
        self.assertIn("pnl=12.34", text)
        self.assertIn("Δ+0.56", text)
        self.assertIn("BEST RULE COMBO | Best Combo", text)
        self.assertIn("POSITION PNL (2)", text)
        self.assertIn("Starmer out by...? YES", text)
        self.assertIn("Δ+0.21", text)
        self.assertIn("bundle=3.0", text)
        self.assertIn("total=0.850", text)
        self.assertIn("net=0.015", text)
        self.assertIn("p submit", text)
        self.assertIn("FUNNEL [paged]", text)
        self.assertIn("DRAFT BUY 3.0 @ 0.61", text)
        self.assertIn("LINK", text)
        self.assertIn("FLOW", text)
        self.assertIn("repeat=842", text)

    def test_render_snapshot_lines_shows_last_full_combo_reference_on_hot_cycle(self) -> None:
        snapshot = {
            "iteration": 6,
            "updated_at": "2026-04-08T09:10:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "realtime_status": "error",
            "realtime_reason": "missing websocket auth",
            "scan_mode": "hot",
            "scan_limit": 500,
            "hot_limit": 200,
            "full_scan_every": 5,
            "scan_offset": 200,
            "reference_candidate_count": 1,
            "reference_best_gross_edge": 0.032,
            "scan_latency_ms": 1800,
            "event_count": 200,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 10,
            "best_gross_edge": -0.145,
            "best_adjusted_edge": -0.175,
            "reference_order_draft": {
                "source": "rule",
                "title": "Best Event",
                "legs": [
                    {"token_id": "t1", "side": "BUY", "price": 0.20, "size": 5.0},
                    {"token_id": "t2", "side": "BUY", "price": 0.30, "size": 5.0},
                ],
                "bundle_size": 5.0,
                "total_price": 0.50,
                "net_edge": 0.01,
            },
            "candidates": [],
            "rejections": [],
            "rejection_reason_counts": {},
            "watched_events": [],
        }

        text = "\n".join(render_snapshot_lines(snapshot, width=140, height=24))

        self.assertIn("realtime=error", text.lower())
        self.assertIn("missing websocket auth", text.lower())
        self.assertIn("refFull=1@+0.032", text)
        self.assertIn("LAST FULL REFERENCE", text)
        self.assertIn("LAST FULL COMBO | Best Event", text)
        self.assertIn("total=0.500", text)

    def test_render_snapshot_lines_can_scroll_candidate_window(self) -> None:
        candidates = [
            {
                "title": f"Item {i:02d}",
                "gross_edge": 0.05 - i * 0.001,
                "adjusted_edge": 0.02 - i * 0.001,
            }
            for i in range(1, 13)
        ]
        snapshot = {
            "iteration": 3,
            "updated_at": "2026-04-08T08:40:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "scan_latency_ms": 2200,
            "event_count": 50,
            "complete_event_count": 0,
            "candidate_count": 12,
            "rejection_count": 50,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": candidates,
            "rejection_reason_counts": {"unsupported_structure": 44},
            "category_counts": {"Politics": 26},
            "structure_counts": {"unsupported_structure": 44},
            "watched_events": [],
        }

        lines = render_snapshot_lines(snapshot, width=120, height=20, watched_offset=5)
        text = "\n".join(lines)

        self.assertIn("CANDIDATE QUEUE", text)
        self.assertIn("Item 06", text)
        self.assertNotIn("Item 01", text)

    def test_render_snapshot_lines_marks_current_focus_row(self) -> None:
        snapshot = {
            "iteration": 6,
            "updated_at": "2026-04-08T10:00:00+00:00",
            "data_source": "gamma-api",
            "source_status": "ok",
            "scan_latency_ms": 1500,
            "event_count": 10,
            "complete_event_count": 0,
            "candidate_count": 2,
            "rejection_count": 10,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "candidates": [
                {"title": "Item 01", "gross_edge": 0.05, "adjusted_edge": 0.02},
                {"title": "Item 02", "gross_edge": 0.04, "adjusted_edge": 0.01},
            ],
            "rejections": [],
            "rejection_reason_counts": {"unsupported_structure": 8},
            "category_counts": {"Politics": 6},
            "structure_counts": {"unsupported_structure": 8},
            "watched_events": [],
        }

        lines = render_snapshot_lines(snapshot, width=120, height=22)
        text = "\n".join(lines)

        self.assertIn("▶1", text)

    def test_render_snapshot_lines_supports_account_only_view(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "2026-04-09T08:00:00+00:00",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 90,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "signer_address": "0x11084005d88a0840b5f38f8731cca9152bbd99f7",
                "funder_address": "0x97c59bce45c910f80cae2a84ee2c8ac0d0069c9f",
                "reason": "clob account api connected",
            },
            "account_snapshot": {
                "balances": {"balance": "123.45"},
                "open_orders_count": 2,
                "open_orders": [
                    {"title": "Election A", "outcome": "YES", "side": "BUY", "price": "0.51", "original_size": "8", "status": "OPEN"},
                ],
                "pnl_summary": {
                    "estimated_total_pnl": 3.21,
                    "estimated_equity": 11.11,
                    "fees_paid": 0.09,
                    "open_position_count": 1,
                },
                "positions": [
                    {"title": "Election A", "outcome": "YES", "net_quantity": 8, "mark_price": 0.62, "estimated_pnl": 0.88},
                ],
                "recent_trades": [
                    {"title": "Election A", "outcome": "YES", "side": "BUY", "size": "8", "price": "0.50"},
                ],
            },
        }

        lines = render_snapshot_lines(snapshot, width=140, height=24)
        text = "\n".join(lines)

        self.assertIn("Polymarket Account [OK]", text)
        self.assertIn("ACCOUNT OVERVIEW", text)
        self.assertIn("ACCOUNT STATUS", text)
        self.assertIn("OPEN POSITIONS", text)
        self.assertIn("RECENT TRADES", text)
        self.assertIn("OPEN ORDERS (1)", text)
        self.assertIn("Election A YES", text)
        self.assertIn("BUY 8 @ 0.50", text)
        self.assertIn("OPEN 8 @ 0.51", text)
        self.assertIn("sigType=2", text)
        self.assertIn("balance=123.45 openOrders=1 | estPnL=3.21", text)
        self.assertNotIn("WATCHED EVENTS", text)

    def test_render_snapshot_lines_account_only_zero_state_shows_balance_and_empty_messages(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "account-1",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 50,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "signer_address": "0x11084005d88a0840b5f38f8731cca9152bbd99f7",
                "funder_address": "0x97c59bce45c910f80cae2a84ee2c8ac0d0069c9f",
                "reason": "clob account api connected",
            },
            "account_snapshot": {
                "balances": {
                    "balance": "0",
                    "allowances": {
                        "0xabc": "1",
                        "0xdef": "2",
                        "0xghi": "3",
                    },
                },
                "open_orders_count": 0,
                "open_orders": [],
                "pnl_summary": {
                    "estimated_total_pnl": 0,
                    "estimated_equity": 0,
                    "fees_paid": 0,
                    "open_position_count": 0,
                },
                "positions": [],
                "recent_trades": [],
            },
        }

        lines = render_snapshot_lines(snapshot, width=140, height=24)
        text = "\n".join(lines)

        self.assertIn("Polymarket Account [OK]", text)
        self.assertIn("ACCOUNT OVERVIEW", text)
        self.assertIn("activity=empty", text)
        self.assertIn("balance=0 openOrders=0 | estPnL=0.00", text)
        self.assertIn("allowances=3", text)
        self.assertIn("OPEN ORDERS (0)", text)
        self.assertIn("No open orders", text)
        self.assertIn("No open positions", text)
        self.assertIn("No recent trades", text)
        self.assertNotIn("sort: top-left", text)
        self.assertIn("controls: Tab focus | j/k scroll | PgUp/PgDn jump | p submit | Ctrl-C exit", text)
        self.assertGreaterEqual(text.count("╰"), 5)

    def test_render_snapshot_lines_account_only_supports_pm_account_snapshot_shape(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "2026-04-09T08:00:00+00:00",
            "data_source": "private-account-api",
            "source_status": "ok",
            "scan_latency_ms": 120,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "reason": "private account api connected",
            },
            "account_snapshot": {
                "balances": {
                    "currentBalance": {"value": "123.45"},
                    "buyingPower": {"value": "67.89"},
                    "openOrders": {"value": "10.00"},
                    "unsettledFunds": {"value": "1.23"},
                },
                "open_orders": {
                    "orders": [
                        {"title": "Election A", "outcome": "YES", "side": "BUY", "price": "0.51", "original_size": "8", "status": "OPEN"},
                    ]
                },
                "activities": {
                    "data": [
                        {"title": "Election A", "outcome": "YES", "side": "BUY", "size": "8", "price": "0.50"},
                    ]
                },
                "positions": [],
                "pnl_summary": {
                    "estimated_total_pnl": 0.0,
                    "estimated_equity": 0.0,
                    "fees_paid": 0.0,
                    "open_position_count": 0,
                },
            },
        }

        lines = render_snapshot_lines(snapshot, width=140, height=24)
        text = "\n".join(lines)

        self.assertIn("balance=123.45", text)
        self.assertIn("openOrders=1", text)
        self.assertIn("recentTrades=1", text)
        self.assertIn("OPEN ORDERS (1)", text)
        self.assertIn("RECENT TRADES (1)", text)
        self.assertIn("balance=123.45 buyingPower=67.89 openOrders=1", text)
        self.assertIn("openOrderFunds=10.00 unsettled=1.23", text)
        self.assertIn("Election A YES | OPEN 8 @ 0.51 | BUY", text)
        self.assertIn("Election A YES | BUY 8 @ 0.50", text)

    def test_render_snapshot_lines_account_only_compact_layout_uses_consistent_titles(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "account-1",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 50,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "reason": "clob account api connected",
            },
            "account_snapshot": {
                "balances": {"balance": "0", "allowances": {"0xabc": "1"}},
                "open_orders_count": 1,
                "open_orders": [
                    {"title": "Election A", "outcome": "YES", "side": "BUY", "price": "0.51", "original_size": "8", "status": "OPEN"},
                ],
                "pnl_summary": {
                    "estimated_total_pnl": 0,
                    "estimated_equity": 0,
                    "fees_paid": 0,
                    "open_position_count": 0,
                },
                "positions": [],
                "recent_trades": [
                    {"title": "Election A", "outcome": "YES", "side": "BUY", "size": "8", "price": "0.50"},
                ],
            },
        }

        lines = render_snapshot_lines(snapshot, width=100, height=24)
        text = "\n".join(lines)

        self.assertIn("ACCOUNT OVERVIEW", text)
        self.assertIn("ACCOUNT STATUS", text)
        self.assertIn("OPEN ORDERS (1)", text)
        self.assertIn("OPEN POSITIONS (0)", text)
        self.assertIn("RECENT TRADES (1)", text)
        self.assertNotIn("Account Status", text)
        self.assertNotIn("Open Orders", text)

    def test_render_snapshot_lines_account_only_compact_live_shape_keeps_recent_trades_visible(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "account-1",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 50,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "signer_address": "0x11084005d88a0840b5f38f8731cca9152bbd99f7",
                "funder_address": "0x97c59bce45c910f80cae2a84ee2c8ac0d0069c9f",
                "reason": "clob account api connected",
            },
            "account_snapshot": {
                "balances": {
                    "balance": "0",
                    "allowances": {
                        "0xabc": "1",
                        "0xdef": "2",
                        "0xghi": "3",
                    },
                },
                "open_orders_count": 0,
                "open_orders": [],
                "pnl_summary": {
                    "estimated_total_pnl": 0,
                    "estimated_equity": 0,
                    "fees_paid": 0,
                    "open_position_count": 0,
                },
                "positions": [],
                "recent_trades": [],
            },
        }

        lines = render_snapshot_lines(snapshot, width=100, height=24)
        text = "\n".join(lines)

        self.assertIn("ACCOUNT STATUS", text)
        self.assertIn("OPEN ORDERS (0)", text)
        self.assertIn("OPEN POSITIONS (0)", text)
        self.assertIn("RECENT TRADES (0)", text)
        self.assertIn("No recent trades", text)

    def test_render_snapshot_lines_account_only_shows_order_draft_and_result(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "account-1",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 50,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {"mode": "account-live", "creds_present": True, "sdk_available": True},
            "account_snapshot": {"balances": {"balance": "0"}, "open_orders": [], "positions": [], "recent_trades": [], "pnl_summary": {"estimated_total_pnl": 0, "estimated_equity": 0, "fees_paid": 0, "open_position_count": 0}},
            "order_draft": ManualOrderConfig(token_id="token-1234567890", side="BUY", price=0.42, size=5.0, order_type="GTC"),
            "order_result": ManualOrderResult(success=True, reason="submitted", order_id="ord-123"),
        }

        lines = render_snapshot_lines(snapshot, width=100, height=24)
        text = "\n".join(lines)

        self.assertIn("draft=BUY 5.0 @ 0.42 | GTC | token=token-123456", text)
        self.assertIn("submit=ok | order=ord-123", text)
        self.assertIn("p submit", text)

    def test_account_only_keyboard_state_scrolls_focused_section_and_cycles_focus(self) -> None:
        snapshot = {
            "view": "account",
            "account_snapshot": {
                "open_orders": [{"title": "o1"}, {"title": "o2"}, {"title": "o3"}],
                "positions": [{"title": "p1"}, {"title": "p2"}],
                "recent_trades": [{"title": "t1"}, {"title": "t2"}, {"title": "t3"}],
            },
        }
        section = "orders"
        offsets = {"orders": 0, "positions": 0, "trades": 0}

        section, offsets, _ = _handle_account_key(ord("j"), snapshot, section, offsets)
        self.assertEqual(section, "orders")
        self.assertEqual(offsets["orders"], 1)

        section, offsets, _ = _handle_account_key(ord("\t"), snapshot, section, offsets)
        self.assertEqual(section, "positions")

        section, offsets, _ = _handle_account_key(ord("j"), snapshot, section, offsets)
        self.assertEqual(offsets["positions"], 1)

        section, offsets, _ = _handle_account_key(ord("\t"), snapshot, section, offsets)
        self.assertEqual(section, "trades")

        section, offsets, _ = _handle_account_key(338, snapshot, section, offsets)  # KEY_NPAGE
        self.assertEqual(offsets["trades"], 2)

    def test_render_snapshot_lines_account_only_offsets_affect_visible_rows(self) -> None:
        snapshot = {
            "view": "account",
            "iteration": 1,
            "updated_at": "account-1",
            "data_source": "clob-account",
            "source_status": "ok",
            "scan_latency_ms": 50,
            "event_count": 0,
            "complete_event_count": 0,
            "candidate_count": 0,
            "rejection_count": 0,
            "best_gross_edge": 0.0,
            "best_adjusted_edge": 0.0,
            "account_status": {
                "mode": "account-live",
                "creds_present": True,
                "sdk_available": True,
                "signature_type": 2,
                "funder_present": True,
                "private_key_var": "CLOB_PRIVATE_KEY",
                "reason": "clob account api connected",
            },
            "account_snapshot": {
                "balances": {"balance": "0", "allowances": {"0xabc": "1"}},
                "open_orders": [
                    {"title": "Order 1", "status": "OPEN", "original_size": "1", "price": "0.1", "side": "BUY"},
                    {"title": "Order 2", "status": "OPEN", "original_size": "1", "price": "0.2", "side": "BUY"},
                ],
                "positions": [
                    {"title": "Pos 1", "net_quantity": 1, "mark_price": 0.1, "estimated_pnl": 0.0},
                    {"title": "Pos 2", "net_quantity": 2, "mark_price": 0.2, "estimated_pnl": 0.0},
                ],
                "recent_trades": [
                    {"title": "Trade 1", "side": "BUY", "size": "1", "price": "0.1"},
                    {"title": "Trade 2", "side": "BUY", "size": "1", "price": "0.2"},
                ],
                "pnl_summary": {"estimated_total_pnl": 0, "estimated_equity": 0, "fees_paid": 0, "open_position_count": 2},
            },
        }

        lines = render_snapshot_lines(
            snapshot,
            width=100,
            height=24,
            account_section="orders",
            account_offsets={"orders": 1, "positions": 1, "trades": 1},
        )
        text = "\n".join(lines)

        self.assertIn("▶ OPEN ORDERS (2)", text)
        self.assertIn("Order 2", text)
        self.assertIn("Pos 2", text)
        self.assertIn("Trade 2", text)
        self.assertNotIn("Order 1", text)

    def test_app_render_snapshot_lines_accepts_account_scroll_args(self) -> None:
        snapshot = {
            "view": "account",
            "account_status": {"mode": "account-live", "creds_present": True, "sdk_available": True},
            "account_snapshot": {
                "balances": {"balance": "0"},
                "open_orders": [{"title": "Order 1", "status": "OPEN", "original_size": "1", "price": "0.1", "side": "BUY"}],
                "positions": [],
                "recent_trades": [],
                "pnl_summary": {"estimated_total_pnl": 0, "estimated_equity": 0, "fees_paid": 0, "open_position_count": 0},
            },
        }

        lines = app_render_snapshot_lines(
            snapshot,
            width=100,
            height=24,
            account_section="orders",
            account_offsets={"orders": 0, "positions": 0, "trades": 0},
        )
        text = "\n".join(lines)

        self.assertIn("▶ OPEN ORDERS (1)", text)
