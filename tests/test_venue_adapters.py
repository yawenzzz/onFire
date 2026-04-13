import unittest

from polymarket_arb.data.contracts import PreviewPayload
from polymarket_arb.venue.adapters import RealPreviewAdapter, RealOrderAdapter, RealWebSocketAdapter
from polymarket_arb.venue.order_client import OrderClient
from polymarket_arb.venue.preview_client import PreviewClient
from polymarket_arb.venue.websocket_client import WebSocketClient


class VenueAdaptersTests(unittest.TestCase):
    def test_preview_adapter_uses_preview_client(self) -> None:
        adapter = RealPreviewAdapter(PreviewClient())
        result = adapter.preview(PreviewPayload(price=0.5, quantity=1.0, side="BUY"), market_open=True, tick_valid=True)
        self.assertTrue(result.ok)

    def test_order_adapter_uses_order_client(self) -> None:
        adapter = RealOrderAdapter(OrderClient())
        result = adapter.place(price=0.5, quantity=1.0, preview_ok=True, market_open=True)
        self.assertTrue(result.accepted)

    def test_websocket_adapter_exposes_freshness_check(self) -> None:
        adapter = RealWebSocketAdapter(WebSocketClient(freshness_budget_ms=500))
        self.assertFalse(adapter.is_fresh(last_message_age_ms=600))
