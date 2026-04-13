from __future__ import annotations

from collections import defaultdict
from typing import Any


def _num(value: Any, default: float = 0.0) -> float:
    if value is None:
        return default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _text(record: dict[str, Any], *keys: str, default: str = "") -> str:
    for key in keys:
        value = record.get(key)
        if value not in (None, ""):
            return str(value)
    return default


def _trade_signed_quantity(trade: dict[str, Any]) -> float:
    qty = _num(_text(trade, "size", "quantity", default="0"))
    side = _text(trade, "side", default="").upper()
    return -qty if side == "SELL" else qty


def _trade_cash_flow(trade: dict[str, Any]) -> float:
    qty = _num(_text(trade, "size", "quantity", default="0"))
    price = _num(_text(trade, "price", default="0"))
    fee = _num(_text(trade, "fee", default="0"))
    notional = qty * price
    side = _text(trade, "side", default="").upper()
    gross = notional if side == "SELL" else -notional
    return gross - fee


def build_clob_account_snapshot(client, trade_limit: int = 50, order_limit: int = 20) -> dict[str, Any]:
    try:
        from py_clob_client.clob_types import AssetType, BalanceAllowanceParams
    except Exception:
        balances = client.get_balance_allowance()
    else:
        balances = client.get_balance_allowance(
            BalanceAllowanceParams(asset_type=AssetType.COLLATERAL)
        )
    open_orders = list(client.get_orders())[:order_limit]
    trades = list(client.get_trades())[:trade_limit]

    grouped: dict[str, dict[str, Any]] = defaultdict(lambda: {
        "asset_id": "",
        "market": "",
        "title": "",
        "outcome": "",
        "net_quantity": 0.0,
        "net_cash_flow": 0.0,
        "fees_paid": 0.0,
        "trade_count": 0,
    })

    for trade in trades:
        asset_id = _text(trade, "asset_id", "assetId")
        bucket = grouped[asset_id]
        bucket["asset_id"] = asset_id
        bucket["market"] = _text(trade, "market", "market_slug")
        bucket["title"] = _text(trade, "title", "market_title")
        bucket["outcome"] = _text(trade, "outcome")
        bucket["net_quantity"] += _trade_signed_quantity(trade)
        bucket["net_cash_flow"] += _trade_cash_flow(trade)
        bucket["fees_paid"] += _num(_text(trade, "fee", default="0"))
        bucket["trade_count"] += 1

    positions: list[dict[str, Any]] = []
    for asset_id, bucket in grouped.items():
        if abs(bucket["net_quantity"]) < 1e-9:
            continue
        mark_price = _num(client.get_midpoint(asset_id), 0.0)
        estimated_equity = bucket["net_quantity"] * mark_price
        estimated_pnl = bucket["net_cash_flow"] + estimated_equity
        positions.append(
            {
                **bucket,
                "mark_price": mark_price,
                "estimated_equity": estimated_equity,
                "estimated_pnl": estimated_pnl,
            }
        )

    positions.sort(key=lambda item: abs(item["estimated_equity"]), reverse=True)
    recent_trades = sorted(trades, key=lambda item: _num(item.get("timestamp"), 0.0), reverse=True)[:10]

    pnl_summary = {
        "fees_paid": round(sum(item["fees_paid"] for item in positions), 6),
        "net_cash_flow": round(sum(item["net_cash_flow"] for item in positions), 6),
        "estimated_equity": round(sum(item["estimated_equity"] for item in positions), 6),
        "estimated_total_pnl": round(sum(item["estimated_pnl"] for item in positions), 6),
        "open_position_count": len(positions),
    }

    return {
        "balances": balances,
        "open_orders_count": len(open_orders),
        "open_orders": open_orders,
        "recent_trades": recent_trades,
        "positions": positions,
        "pnl_summary": pnl_summary,
    }
