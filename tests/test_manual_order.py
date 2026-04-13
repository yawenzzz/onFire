import tempfile
import unittest
from pathlib import Path

from polymarket_arb.auth.manual_order import (
    ManualOrderConfig,
    ManualOrderResult,
    load_manual_order_config,
    submit_manual_order,
)


class StubClient:
    def __init__(self) -> None:
        self.created = None
        self.posted = None
        self.posted_many = None

    def create_order(self, order_args):
        self.created = order_args
        return {"signed": True, "token_id": order_args.token_id}

    def post_order(self, order, orderType="GTC", post_only=False):
        self.posted = {"order": order, "orderType": orderType, "post_only": post_only}
        return {"success": True, "orderID": "ord-123"}

    def post_orders(self, args):
        self.posted_many = args
        return [{"id": "ord-batch-1"}]


class ManualOrderTests(unittest.TestCase):
    def test_loads_manual_order_config_from_env_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "ORDER_TOKEN_ID=token-1\n"
                "ORDER_SIDE=BUY\n"
                "ORDER_PRICE=0.42\n"
                "ORDER_SIZE=5\n"
                "ORDER_TYPE=GTC\n"
            )
            cfg = load_manual_order_config(root)

        self.assertIsNotNone(cfg)
        assert cfg is not None
        self.assertEqual(cfg.token_id, "token-1")
        self.assertEqual(cfg.side, "BUY")
        self.assertEqual(cfg.price, 0.42)
        self.assertEqual(cfg.size, 5.0)
        self.assertEqual(cfg.order_type, "GTC")

    def test_submit_manual_order_uses_client_create_and_post(self) -> None:
        client = StubClient()
        cfg = ManualOrderConfig(
            token_id="token-1",
            side="BUY",
            price=0.42,
            size=5.0,
            order_type="GTC",
        )

        result = submit_manual_order(cfg, client_factory=lambda: client)

        self.assertIsInstance(result, ManualOrderResult)
        self.assertTrue(result.success)
        self.assertEqual(result.order_id, "ord-123")
        self.assertEqual(client.created.token_id, "token-1")
        self.assertEqual(client.posted["orderType"], "GTC")

    def test_submit_manual_order_returns_error_for_missing_config(self) -> None:
        result = submit_manual_order(None, client_factory=lambda: StubClient())
        self.assertFalse(result.success)
        self.assertIn("draft", result.reason)

    def test_invalid_numeric_order_config_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "ORDER_TOKEN_ID=token-1\n"
                "ORDER_SIDE=BUY\n"
                "ORDER_PRICE=not-a-number\n"
                "ORDER_SIZE=5\n"
            )
            cfg = load_manual_order_config(root)

        self.assertIsNone(cfg)

    def test_submit_rule_bundle_uses_batch_post(self) -> None:
        client = StubClient()
        draft = {
            "order_type": "GTC",
            "legs": [
                {"token_id": "token-1", "side": "BUY", "price": 0.42, "size": 5.0},
                {"token_id": "token-2", "side": "BUY", "price": 0.33, "size": 5.0},
            ],
        }

        result = submit_manual_order(draft, client_factory=lambda: client)

        self.assertTrue(result.success)
        self.assertEqual(result.order_id, "ord-batch-1")
        self.assertEqual(len(client.posted_many), 2)
