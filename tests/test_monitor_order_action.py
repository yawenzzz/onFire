import unittest

from polymarket_arb.app.monitor_tui import _handle_account_key, _handle_monitor_key


class MonitorOrderActionTests(unittest.TestCase):
    def test_account_key_returns_submit_action(self) -> None:
        snapshot = {"view": "account", "account_snapshot": {"open_orders": [], "positions": [], "recent_trades": []}}
        section, offsets, action = _handle_account_key(
            ord("p"),
            snapshot,
            "orders",
            {"orders": 0, "positions": 0, "trades": 0},
        )

        self.assertEqual(section, "orders")
        self.assertEqual(offsets["orders"], 0)
        self.assertEqual(action, {"type": "submit_order"})

    def test_monitor_key_returns_submit_action_for_rule_draft(self) -> None:
        snapshot = {"order_draft": {"source": "rule", "title": "Best Event"}}
        watched_offset, action = _handle_monitor_key(ord("p"), snapshot, 0)

        self.assertEqual(watched_offset, 0)
        self.assertEqual(action, {"type": "submit_order"})
