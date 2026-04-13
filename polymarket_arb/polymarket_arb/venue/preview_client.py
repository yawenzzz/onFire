from __future__ import annotations

from dataclasses import dataclass


@dataclass
class PreviewResult:
    ok: bool
    reason: str | None = None


class PreviewClient:
    def preview(self, price: float, tick_valid: bool, market_open: bool) -> PreviewResult:
        if not market_open:
            return PreviewResult(ok=False, reason="market not open")
        if not tick_valid:
            return PreviewResult(ok=False, reason="invalid tick")
        if not 0.01 <= price <= 0.99:
            return PreviewResult(ok=False, reason="price out of bounds")
        return PreviewResult(ok=True, reason=None)
