from __future__ import annotations

from polymarket_arb.data.contracts import PreviewPayload


class RealPreviewAdapter:
    def __init__(self, client) -> None:
        self.client = client

    def preview(self, payload: PreviewPayload, market_open: bool, tick_valid: bool):
        return self.client.preview(price=payload.price, tick_valid=tick_valid, market_open=market_open)


class RealOrderAdapter:
    def __init__(self, client) -> None:
        self.client = client

    def place(self, price: float, quantity: float, preview_ok: bool, market_open: bool):
        request_cls = getattr(__import__('polymarket_arb.venue.order_client', fromlist=['OrderRequest']), 'OrderRequest')
        request = request_cls(price=price, quantity=quantity, preview_ok=preview_ok, market_open=market_open)
        return self.client.place(request)


class RealWebSocketAdapter:
    def __init__(self, client) -> None:
        self.client = client

    def is_fresh(self, last_message_age_ms: int) -> bool:
        return self.client.is_fresh(last_message_age_ms=last_message_age_ms)
