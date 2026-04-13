import unittest

from polymarket_arb.monitoring.clob_account_snapshot import build_clob_account_snapshot


class StubClobClient:
    def __init__(self) -> None:
        self.midpoints = {
            "token-a": 0.62,
            "token-b": 0.40,
        }
        self.balance_params = None

    def get_balance_allowance(self, params=None):
        self.balance_params = params
        return {
            "balance": "150.25",
            "allowances": {"USDC": "150.25"},
        }

    def get_orders(self, params=None, next_cursor="MA=="):
        return [
            {
                "id": "order-1",
                "market": "event-a",
                "asset_id": "token-a",
                "side": "BUY",
                "price": "0.55",
                "original_size": "10",
                "status": "OPEN",
                "title": "Election A",
                "outcome": "YES",
            }
        ]

    def get_trades(self, params=None, next_cursor="MA=="):
        return [
            {
                "id": "trade-1",
                "market": "event-a",
                "asset_id": "token-a",
                "side": "BUY",
                "price": "0.50",
                "size": "10",
                "fee_rate_bps": "0",
                "title": "Election A",
                "outcome": "YES",
                "timestamp": 1700000001000,
            },
            {
                "id": "trade-2",
                "market": "event-a",
                "asset_id": "token-a",
                "side": "SELL",
                "price": "0.70",
                "size": "4",
                "fee": "0.02",
                "title": "Election A",
                "outcome": "YES",
                "timestamp": 1700000002000,
            },
            {
                "id": "trade-3",
                "market": "event-b",
                "asset_id": "token-b",
                "side": "BUY",
                "price": "0.30",
                "size": "5",
                "fee": "0.01",
                "title": "Election B",
                "outcome": "NO",
                "timestamp": 1700000003000,
            },
        ]

    def get_midpoint(self, token_id):
        return self.midpoints[token_id]


class ClobAccountSnapshotTests(unittest.TestCase):
    def test_passes_balance_allowance_params_object(self) -> None:
        client = StubClobClient()
        build_clob_account_snapshot(client, trade_limit=10)
        self.assertIsNotNone(client.balance_params)
        self.assertEqual(str(client.balance_params.asset_type), "COLLATERAL")

    def test_builds_balances_positions_orders_and_recent_trades(self) -> None:
        snapshot = build_clob_account_snapshot(StubClobClient(), trade_limit=10)

        self.assertEqual(snapshot["balances"]["balance"], "150.25")
        self.assertEqual(snapshot["open_orders_count"], 1)
        self.assertEqual(len(snapshot["open_orders"]), 1)
        self.assertEqual(len(snapshot["recent_trades"]), 3)
        self.assertEqual(len(snapshot["positions"]), 2)

        first = snapshot["positions"][0]
        self.assertEqual(first["asset_id"], "token-a")
        self.assertEqual(first["net_quantity"], 6.0)
        self.assertAlmostEqual(first["mark_price"], 0.62)
        self.assertAlmostEqual(first["estimated_pnl"], 1.50, places=2)

    def test_aggregates_total_estimated_pnl_and_fees(self) -> None:
        snapshot = build_clob_account_snapshot(StubClobClient(), trade_limit=10)
        pnl = snapshot["pnl_summary"]

        self.assertAlmostEqual(pnl["fees_paid"], 0.03, places=2)
        self.assertAlmostEqual(pnl["net_cash_flow"], -3.73, places=2)
        self.assertAlmostEqual(pnl["estimated_equity"], 5.72, places=2)
        self.assertAlmostEqual(pnl["estimated_total_pnl"], 1.99, places=2)
