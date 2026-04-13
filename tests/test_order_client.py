import unittest

from polymarket_arb.venue.order_client import OrderClient, OrderRequest, OrderResponse


class OrderClientTests(unittest.TestCase):
    def test_rejects_order_when_preview_not_ok(self) -> None:
        client = OrderClient()
        response = client.place(OrderRequest(price=0.5, quantity=1.0, preview_ok=False, market_open=True))
        self.assertIsInstance(response, OrderResponse)
        self.assertFalse(response.accepted)
        self.assertEqual(response.reason, "preview not ok")

    def test_accepts_order_when_inputs_are_safe(self) -> None:
        client = OrderClient()
        response = client.place(OrderRequest(price=0.5, quantity=1.0, preview_ok=True, market_open=True))
        self.assertTrue(response.accepted)
        self.assertIsNone(response.reason)
