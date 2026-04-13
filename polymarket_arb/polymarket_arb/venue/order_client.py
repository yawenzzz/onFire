from __future__ import annotations

from dataclasses import dataclass


@dataclass
class OrderRequest:
    price: float
    quantity: float
    preview_ok: bool
    market_open: bool


@dataclass
class OrderResponse:
    accepted: bool
    reason: str | None = None


class OrderClient:
    def place(self, request: OrderRequest) -> OrderResponse:
        if not request.preview_ok:
            return OrderResponse(accepted=False, reason="preview not ok")
        if not request.market_open:
            return OrderResponse(accepted=False, reason="market not open")
        if request.quantity <= 0:
            return OrderResponse(accepted=False, reason="quantity must be positive")
        if not 0.01 <= request.price <= 0.99:
            return OrderResponse(accepted=False, reason="price out of bounds")
        return OrderResponse(accepted=True, reason=None)
