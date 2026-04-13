from __future__ import annotations

from decimal import Decimal

from polymarket_arb.monitoring.economics import fee_adjusted_bundle_cost, fee_rate_for_candidate


DEFAULT_FEE_BUFFER = 0.02
DEFAULT_SLIPPAGE_BUFFER = 0.005
DEFAULT_SAFETY_BUFFER = 0.005


def _read(obj, key, default=None):
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def _recommended_orders(candidate: dict) -> list[dict]:
    orders = _read(candidate, "recommended_orders", []) or []
    return [dict(item) for item in orders if item.get("token_id") and item.get("price") is not None]


def _bundle_size(orders: list[dict]) -> float:
    if not orders:
        return 0.0
    sizes = []
    for item in orders:
        depth = item.get("depth_ask_top")
        minimum = item.get("order_min_size")
        depth_val = float(depth) if depth not in (None, "") else float(minimum or 1.0)
        minimum_val = float(minimum) if minimum not in (None, "") else 1.0
        sizes.append(max(minimum_val, min(depth_val, minimum_val)))
    return max(sizes) if sizes else 0.0


def _net_edge(candidate: dict, total_price: float) -> float:
    gross_edge = float(_read(candidate, "gross_edge", 0.0) or 0.0)
    fee_buffer = float(_read(candidate, "fee_buffer", DEFAULT_FEE_BUFFER))
    slippage_buffer = float(_read(candidate, "slippage_buffer", DEFAULT_SLIPPAGE_BUFFER))
    safety_buffer = float(_read(candidate, "safety_buffer", DEFAULT_SAFETY_BUFFER))
    fee_rate = fee_rate_for_candidate(candidate)
    size = Decimal(str(_bundle_size(_recommended_orders(candidate)) or 0.0))
    prices = [Decimal(str(item.get("price") or 0.0)) for item in _recommended_orders(candidate)]
    fee_adjusted_cost = fee_adjusted_bundle_cost(size, prices, fee_rate) if size > 0 and prices else Decimal("0")
    raw_cost = size * sum(prices, Decimal("0"))
    fee_impact = float((fee_adjusted_cost - raw_cost) / size) if size > 0 else 0.0
    return round(gross_edge - fee_impact - fee_buffer - slippage_buffer - safety_buffer, 6)


def recommend_order_draft(snapshot: dict) -> dict | None:
    candidates = _read(snapshot, "candidates", []) or []
    viable: list[dict] = []
    for candidate in candidates:
        orders = _recommended_orders(candidate)
        if not orders:
            continue
        size = _bundle_size(orders)
        if size <= 0:
            continue
        for order in orders:
            order["size"] = size
        total_price = round(sum(float(item.get("price") or 0.0) for item in orders), 6)
        net_edge = _net_edge(candidate, total_price)
        fee_rate = float(fee_rate_for_candidate(candidate))
        size_dec = Decimal(str(size))
        prices = [Decimal(str(item.get("price") or 0.0)) for item in orders]
        fee_adjusted_cost = fee_adjusted_bundle_cost(size_dec, prices, Decimal(str(fee_rate))) if size_dec > 0 and prices else Decimal("0")
        raw_cost = size_dec * sum(prices, Decimal("0"))
        viable.append(
            {
                "source": "rule",
                "title": _read(candidate, "title"),
                "template_type": _read(candidate, "template_type"),
                "gross_edge": float(_read(candidate, "gross_edge", 0.0) or 0.0),
                "adjusted_edge": _read(candidate, "adjusted_edge", _read(candidate, "cost_adjusted_edge")),
                "net_edge": net_edge,
                "fee_rate": fee_rate,
                "fee_cost": float(fee_adjusted_cost - raw_cost),
                "legs": orders,
                "order_type": "GTC",
                "bundle_size": size,
                "total_price": total_price,
            }
        )

    viable = [item for item in viable if item["net_edge"] > 0]
    if not viable:
        return None
    viable.sort(key=lambda item: (item["net_edge"], item["gross_edge"]), reverse=True)
    return viable[0]
