from __future__ import annotations

from dataclasses import dataclass


@dataclass
class MarketSnapshot:
    market_id: str
    market_state: str
    best_bid: float
    best_ask: float

    def is_tradeable(self) -> bool:
        return self.market_state == "OPEN" and 0.0 <= self.best_bid <= self.best_ask <= 1.0


@dataclass
class PreviewPayload:
    price: float
    quantity: float
    side: str

    def to_dict(self) -> dict:
        return {"price": self.price, "quantity": self.quantity, "side": self.side}
