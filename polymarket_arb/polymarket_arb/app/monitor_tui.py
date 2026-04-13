from __future__ import annotations

import curses
import io
from datetime import datetime, timezone, timedelta
from typing import Any
from rich.columns import Columns
from rich.console import Console, Group
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich import box
from polymarket_arb.monitoring.order_recommendation import recommend_order_draft


def _read(obj: Any, key: str, default=None):
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def _fit(text: str, width: int) -> str:
    if width <= 0:
        return ""
    if len(text) <= width:
        return text
    if width <= 3:
        return text[:width]
    return text[: width - 3] + "..."


def _pad(text: str, width: int) -> str:
    fitted = _fit(text, width)
    return fitted + (" " * max(0, width - len(fitted)))


def _fmt_num(value: Any, digits: int = 3) -> str:
    if value in (None, ""):
        return "n/a"
    try:
        return f"{float(value):.{digits}f}"
    except (TypeError, ValueError):
        return "n/a"


def _fmt_signed_num(value: Any, digits: int = 2) -> str:
    if value in (None, ""):
        return "n/a"
    try:
        return f"{float(value):+.{digits}f}"
    except (TypeError, ValueError):
        return "n/a"


def _text(obj: Any, *keys: str, default: str = "") -> str:
    for key in keys:
        value = _read(obj, key, None)
        if value not in (None, ""):
            return str(value)
    return default


def _short_struct(value: Any) -> str:
    text = str(value or "n/a")
    mapping = {
        "unsupported_structure": "unsupported",
        "open_candidate_set": "open_set",
        "overlapping_ranges": "overlap_rng",
        "overlapping_thresholds": "overlap_thr",
        "market_cap_buckets": "mcap_bucket",
        "count_complete_bucket": "count_bucket",
        "ipo_market_cap_complete_bucket": "ipo_bucket",
    }
    return mapping.get(text, text[:12])


def _fmt_gmt8(value: Any) -> str:
    if not value:
        return "unknown"
    try:
        text = str(value).replace("Z", "+00:00")
        dt = datetime.fromisoformat(text)
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        dt = dt.astimezone(timezone(timedelta(hours=8)))
        return dt.strftime("%Y-%m-%d %H:%M:%S GMT+8")
    except ValueError:
        return str(value)


def _candidate_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    return _candidate_lines_with_offset(snapshot, width, height, 0)


def _candidate_items(snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    return list(_read(snapshot, "candidates", []) or [])


def _candidate_as_dict(item: Any) -> dict[str, Any]:
    if isinstance(item, dict):
        return item
    return getattr(item, "__dict__", {})


def _candidate_lines_with_offset(snapshot: dict[str, Any], width: int, height: int, offset: int = 0) -> list[str]:
    rows = list(_read(snapshot, "candidates", []) or [])
    title = "TOP CANDIDATES"
    if not rows:
        rows = sorted(
            list(_read(snapshot, "rejections", []) or []),
            key=lambda item: float(_read(item, "gross_edge", 0.0)),
            reverse=True,
        )
        title = "TOP REJECTED EVENTS"
    lines = [_fit(f"{'rk':<2} {'gross':>8} {'adj':>8}  title", width)]
    if not rows:
        lines.append("none")
        return lines[:height]
    start = min(max(0, offset), max(0, len(rows) - 1))
    end = min(len(rows), start + max(1, height - 2))
    for index, item in enumerate(rows[start:end], start=start + 1):
        lines.append(
            _fit(
                f"{index:<2} "
                f"{float(_read(item, 'gross_edge', 0.0)):>+8.3f} "
                f"{float(_read(item, 'adjusted_edge', _read(item, 'cost_adjusted_edge', 0.0))):>+8.3f}  "
                f"{_read(item, 'title', 'unknown')}",
                width,
            )
        )
    return lines[:height]


def _rejection_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    lines = []
    counts = _read(snapshot, "rejection_reason_counts", _read(snapshot, "rejection_counts", {})) or {}
    if not counts:
        lines.append("none")
        return lines[:height]
    for reason, count in sorted(counts.items()):
        lines.append(_fit(f"{reason:<28} {count:>6}", width))
    return lines[:height]


def _watched_lines(snapshot: dict[str, Any], width: int, height: int, offset: int = 0) -> list[str]:
    watched = _read(snapshot, "watched_events", []) or []
    visible = max(1, height - 2)
    start = min(max(0, offset), max(0, len(watched) - 1))
    end = min(len(watched), start + visible)
    lines = [
        _fit(f"showing {start + 1 if watched else 0}-{end}/{len(watched)} | j/k scroll", width),
        _fit(f"{'':<1}{'STATUS':<6}│{'BOARD':<8}│{'PRICE':^13}│{'EVENT':<46}│{'STRUCT':<8}", width),
        _fit(f"{'':<1}{'st':<6}│{'cat':<8}│{'bid':>6} {'ask':>6}│{'title':<46}│{'struct':<8}", width),
    ]
    if not watched:
        lines.append("none")
        return lines[:height]
    for local_index, item in enumerate(watched[start:end]):
        marker = "▶" if local_index == 0 else " "
        lines.append(
            _fit(
                    f"{marker:<1}"
                    f"{_read(item, 'status', '?'):<6}"
                    f"│{_read(item, 'category', 'unknown'):<8}"
                    f"│{_fmt_num(_read(item, 'best_bid'), 3):>6}"
                    f"│{_fmt_num(_read(item, 'best_ask'), 3):>6}"
                    f"│{_read(item, 'title', 'unknown'):<46}"
                    f"│{_short_struct(_read(item, 'structure', 'n/a')):<8}",
                    width,
                )
            )
    return lines[:height]


def _structure_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    counts = _read(snapshot, "structure_counts", {}) or {}
    if not counts:
        return ["STRATEGY BREAKDOWN: none"][:height]
    parts = [f"{name} {count}" for name, count in sorted(counts.items(), key=lambda item: (-item[1], item[0]))[:3]]
    return ([_fit("STRATEGY BREAKDOWN", width)] + [_fit(" | ".join(parts), width)])[:height]


def _category_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    counts = _read(snapshot, "category_counts", {}) or {}
    if not counts:
        return ["CATEGORY BREAKDOWN", "none"][:height]
    lines = ["CATEGORY BREAKDOWN"]
    for name, count in sorted(counts.items(), key=lambda item: (-item[1], item[0]))[: max(1, height - 1)]:
        lines.append(_fit(f"{name:<18} {count:>6}", width))
    return lines[:height]


def _focused_lines(snapshot: dict[str, Any], width: int, height: int, offset: int = 0) -> list[str]:
    watched = _read(snapshot, "watched_events", []) or []
    start = min(max(0, offset), max(0, len(watched) - 1))
    item = watched[start] if watched else None
    lines = [f"{start + 1 if watched else 0}/{len(watched)}"]
    if not item:
        lines.append("none")
        return lines[:height]
    lines.extend(
        [
            _fit(
                f"META  {_read(item, 'status', '?')} | {_read(item, 'category', 'unknown')} | "
                f"{_read(item, 'structure', 'n/a')}",
                width,
            ),
            _fit(
                f"LINK  {_read(item, 'event_url', '-')}",
                width,
            ),
            _fit(
                f"FLOW  vol24={_fmt_num(_read(item, 'volume_24hr'), 0)} "
                f"dBid={_fmt_num(_read(item, 'depth_bid_top'), 0)} dAsk={_fmt_num(_read(item, 'depth_ask_top'), 0)}",
                width,
            ),
            _fit(
                f"TITLE {_read(item, 'title', 'unknown')}",
                width,
            ),
            _fit(
                f"PX    bid={_fmt_num(_read(item, 'best_bid'), 3)} ask={_fmt_num(_read(item, 'best_ask'), 3)} "
                f"last={_fmt_num(_read(item, 'last_trade_price'), 3)}",
                width,
            ),
            _fit(
                f"SRC   {_read(item, 'resolution_source', '-')}",
                width,
            ),
        ]
    )
    return lines[:height]


def _focused_candidate_lines(snapshot: dict[str, Any], width: int, height: int, offset: int = 0) -> list[str]:
    candidates = _candidate_items(snapshot)
    start = min(max(0, offset), max(0, len(candidates) - 1))
    item = candidates[start] if candidates else None
    lines = [f"{start + 1 if candidates else 0}/{len(candidates)}"]
    if not item:
        lines.append("none")
        return lines[:height]
    first_order = (_read(item, "recommended_orders", []) or [None])[0]
    lines.extend(
        [
            _fit(
                f"META  CANDIDATE | {_read(item, 'category', 'unknown')} | {_read(item, 'template_type', 'n/a')}",
                width,
            ),
            _fit(
                f"TITLE {_read(item, 'title', 'unknown')} | "
                f"DRAFT {_text(first_order, 'side', default='?')} {_text(first_order, 'size', default='?')} @ {_text(first_order, 'price', default='?')}",
                width,
            ),
            _fit(
                f"PX    bid={_fmt_num(_read(item, 'best_bid'), 3)} ask={_fmt_num(_read(item, 'best_ask'), 3)} "
                f"last={_fmt_num(_read(item, 'last_trade_price'), 3)}",
                width,
            ),
            _fit(
                f"FLOW  vol24={_fmt_num(_read(item, 'volume_24hr'), 0)} liq={_fmt_num(_read(item, 'liquidity'), 0)} "
                f"dBid={_fmt_num(_read(item, 'depth_bid_top'), 0)} dAsk={_fmt_num(_read(item, 'depth_ask_top'), 0)} "
                f"repeat={_read(item, 'repeat_interval_ms', 'n/a')}",
                width,
            ),
            _fit(f"LINK  {_read(item, 'event_url', '-')}", width),
        ]
    )
    return lines[:height]


def _rules_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    min_edge = _read(snapshot, "min_edge_threshold", 0.03)
    fee = _read(snapshot, "fee_buffer", 0.02)
    slip = _read(snapshot, "slippage_buffer", 0.005)
    safety = _read(snapshot, "safety_buffer", 0.005)
    lines = [
        _fit("1. open markets only", width),
        _fit("2. supported structure + no overlap/open-set rejection", width),
        _fit(f"3. candidate requires gross_edge >= {_fmt_num(min_edge,3)}", width),
        _fit(f"4. draft requires net_edge > 0 | fee={_fmt_num(fee,3)} slip={_fmt_num(slip,3)} safety={_fmt_num(safety,3)}", width),
        _fit(
            f"scan_limit={_read(snapshot, 'scan_limit', _read(snapshot, 'event_count', 0))} pages={_read(snapshot, 'page_count', 1)}",
            width,
        ),
    ]
    return lines[:height]


def _account_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    account = _read(snapshot, "account_status", {}) or {}
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    if not account:
        return ["mode=public-only | creds=no | sdk=no"][:height]
    lines = [
        _fit(
            f"mode={account.get('mode', 'unknown')} | "
            f"creds={'yes' if account.get('creds_present') else 'no'} | "
            f"sdk={'yes' if account.get('sdk_available') else 'no'}",
            width,
        ),
    ]
    lines.append(
        _fit(
            "sigType={sig_type} | funder={funder} | pk={pk_var}".format(
                sig_type=account.get("signature_type", "n/a"),
                funder="yes" if account.get("funder_present") else "no",
                pk_var=account.get("private_key_var", "none"),
            ),
            width,
        )
    )
    signer = account.get("signer_address")
    funder_address = account.get("funder_address")
    order_count = _open_order_count(account_snapshot) if isinstance(account_snapshot, dict) else 0
    if (signer or funder_address) and _read(snapshot, "view", "") != "account":
        lines[-1] = _fit(
            "{base} | {signer}->{funder}".format(
                base=lines[-1],
                signer=(signer or "n/a")[:6],
                funder=(funder_address or "n/a")[:6],
            ),
            width,
        )
    elif signer or funder_address:
        lines.append(
            _fit(
                "signer={signer} | funderAddr={funder}".format(
                    signer=signer or "n/a",
                    funder=funder_address or "n/a",
                ),
                width,
            )
        )
    balances = account_snapshot.get("balances") if isinstance(account_snapshot, dict) else None
    primary_summary = None
    secondary_summary = None
    if isinstance(balances, dict) and any(key in balances for key in ("currentBalance", "buyingPower", "openOrders", "unsettledFunds")):
        primary_summary = (
            "balance={balance} buyingPower={buying_power} openOrders={open_orders}".format(
                balance=balances.get("currentBalance", {}).get("value", "n/a"),
                buying_power=balances.get("buyingPower", {}).get("value", "n/a"),
                open_orders=order_count,
            )
        )
        secondary_summary = (
            "openOrderFunds={open_order_funds} unsettled={unsettled}".format(
                open_order_funds=balances.get("openOrders", {}).get("value", "n/a"),
                unsettled=balances.get("unsettledFunds", {}).get("value", "n/a"),
            )
        )
    else:
        primary_summary = (
            "balance={balance} openOrders={open_orders_count}".format(
                balance=_text(balances or {}, "balance", default="n/a"),
                open_orders_count=order_count if isinstance(account_snapshot, dict) else "n/a",
            )
        )
    pnl_summary = account_snapshot.get("pnl_summary") if isinstance(account_snapshot, dict) else None
    if isinstance(pnl_summary, dict):
        pnl_delta = pnl_summary.get("estimated_total_pnl_delta")
        primary_summary = ((primary_summary + " | ") if primary_summary else "") + "estPnL={pnl}".format(
            pnl=_fmt_num(pnl_summary.get("estimated_total_pnl"), 2)
        )
        if pnl_delta not in (None, ""):
            primary_summary += f" Δ{_fmt_signed_num(pnl_delta, 2)}"
        secondary_summary = ((secondary_summary + " | ") if secondary_summary else "") + "equity={equity}".format(
            equity=_fmt_num(pnl_summary.get("estimated_equity"), 2),
        )
        equity_delta = pnl_summary.get("estimated_equity_delta")
        if equity_delta not in (None, ""):
            secondary_summary += f" Δ{_fmt_signed_num(equity_delta, 2)}"
        secondary_summary += " fees={fees} pos={positions}".format(
            fees=_fmt_num(pnl_summary.get("fees_paid"), 2),
            positions=pnl_summary.get("open_position_count", 0),
        )
    if _read(snapshot, "view", "") != "account" and primary_summary and secondary_summary:
        primary_summary = f"{primary_summary} | {secondary_summary}"
        secondary_summary = None
    allowances = (balances or {}).get("allowances") if isinstance(balances, dict) else None
    if primary_summary:
        lines.append(_fit(primary_summary, width))
    if isinstance(allowances, dict):
        lines.append(_fit(f"allowances={len(allowances)}", width))
    if secondary_summary and height >= 5:
        lines.append(_fit(secondary_summary, width))
    return lines[:height]


def _positions_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    return _positions_lines_with_offset(snapshot, width, height, 0)


def _positions_lines_with_offset(snapshot: dict[str, Any], width: int, height: int, offset: int) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    positions = account_snapshot.get("positions") if isinstance(account_snapshot, dict) else None
    if not positions:
        return ["No open positions"][:height]
    lines = []
    start = min(max(0, offset), max(0, len(positions) - 1))
    end = min(len(positions), start + height)
    for item in positions[start:end]:
        title = _text(item, "title", default=_text(item, "asset_id", default="position"))
        pnl_delta = _read(item, "estimated_pnl_delta")
        pnl_suffix = f" Δ{_fmt_signed_num(pnl_delta, 2)}" if pnl_delta not in (None, "") else ""
        lines.append(
            _fit(
                f"{title} {_text(item, 'outcome', default='').strip()} qty={_fmt_num(_read(item, 'net_quantity'), 2)} "
                f"mark={_fmt_num(_read(item, 'mark_price'), 3)} pnl={_fmt_num(_read(item, 'estimated_pnl'), 2)}{pnl_suffix}",
                width,
            )
        )
    return lines[:height]


def _position_pnl_lines(snapshot: dict[str, Any], width: int, height: int, limit: int | None = None) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    positions = list(account_snapshot.get("positions") or []) if isinstance(account_snapshot, dict) else []
    if not positions:
        return ["No open positions"][:height]
    positions.sort(
        key=lambda item: (
            abs(float(_read(item, "estimated_pnl", 0.0) or 0.0)),
            abs(float(_read(item, "estimated_pnl_delta", 0.0) or 0.0)),
        ),
        reverse=True,
    )
    if limit is not None:
        positions = positions[:limit]
    lines: list[str] = []
    for item in positions[:height]:
        pnl = _read(item, "estimated_pnl", 0.0)
        pnl_delta = _read(item, "estimated_pnl_delta", None)
        marker = "▲" if float(pnl or 0.0) >= 0 else "▼"
        delta_text = f" Δ{_fmt_signed_num(pnl_delta, 2)}" if pnl_delta not in (None, "") else ""
        lines.append(
            _fit(
                f"{marker} {_fmt_signed_num(pnl, 2)}{delta_text} | qty={_fmt_num(_read(item, 'net_quantity'), 2)} "
                f"@ {_fmt_num(_read(item, 'mark_price'), 3)} | {_text(item, 'title', default='position')} {_text(item, 'outcome', default='').strip()}",
                width,
            )
        )
    return lines[:height]


def _orders_list(account_snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    orders = account_snapshot.get("open_orders") if isinstance(account_snapshot, dict) else None
    if isinstance(orders, list):
        return orders
    if isinstance(orders, dict):
        data = orders.get("orders") or orders.get("data")
        if isinstance(data, list):
            return data
    return []


def _trades_list(account_snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    trades = account_snapshot.get("recent_trades") if isinstance(account_snapshot, dict) else None
    if isinstance(trades, list):
        return trades
    activities = account_snapshot.get("activities", {}) if isinstance(account_snapshot, dict) else {}
    if isinstance(activities, dict):
        data = activities.get("data")
        if isinstance(data, list):
            return data
    return []


def _open_order_count(account_snapshot: dict[str, Any]) -> int:
    return len(_orders_list(account_snapshot))


def _recent_trade_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    return _recent_trade_lines_with_offset(snapshot, width, height, 0)


def _recent_trade_lines_with_offset(snapshot: dict[str, Any], width: int, height: int, offset: int) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    trades = _trades_list(account_snapshot)
    if not trades:
        return ["No recent trades"][:height]
    lines = []
    start = min(max(0, offset), max(0, len(trades) - 1))
    end = min(len(trades), start + height)
    for item in trades[start:end]:
        title = _text(item, "title", "market_title", default="trade")
        side = _text(item, "side", default="?").upper()
        size = _text(item, "size", "quantity", default="?")
        price = _text(item, "price", default="?")
        outcome = _text(item, "outcome", default="")
        suffix = f" {outcome}" if outcome else ""
        lines.append(_fit(f"{title}{suffix} | {side} {size} @ {price}", width))
    return lines[:height]


def _open_order_lines(snapshot: dict[str, Any], width: int, height: int) -> list[str]:
    return _open_order_lines_with_offset(snapshot, width, height, 0)


def _open_order_lines_with_offset(snapshot: dict[str, Any], width: int, height: int, offset: int) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    orders = _orders_list(account_snapshot)
    if not orders:
        return ["No open orders"][:height]
    lines = []
    start = min(max(0, offset), max(0, len(orders) - 1))
    end = min(len(orders), start + height)
    for item in orders[start:end]:
        title = _text(item, "title", default="order")
        outcome = _text(item, "outcome", default="")
        side = _text(item, "side", default="?").upper()
        size = _text(item, "original_size", "size", default="?")
        price = _text(item, "price", default="?")
        status = _text(item, "status", default="OPEN").upper()
        suffix = f" {outcome}" if outcome else ""
        lines.append(_fit(f"{title}{suffix} | {status} {size} @ {price} | {side}", width))
    return lines[:height]


def _box(title: str, content: list[str], width: int, height: int) -> list[str]:
    inner = max(1, width - 2)
    title_line = f"┌ {title} " + "─" * max(0, inner - len(title) - 2) + "┐"
    rows = [title_line[:width]]
    body_height = max(1, height - 2)
    padded = content[:body_height] + [""] * max(0, body_height - len(content))
    for row in padded[:body_height]:
        rows.append(f"│{_pad(row, inner)}│")
    rows.append(f"└{'─' * inner}┘")
    return rows[:height]


def _counted_title(base: str, count: int | None) -> str:
    if count is None:
        return base
    return f"{base} ({count})"


def _focused_title(base: str, count: int | None, focused: bool) -> str:
    title = _counted_title(base, count)
    return f"▶ {title}" if focused else title


def _balance_value(balances: dict[str, Any] | None) -> str:
    if not isinstance(balances, dict):
        return "n/a"
    if "currentBalance" in balances:
        current = balances.get("currentBalance")
        if isinstance(current, dict):
            return _text(current, "value", default="n/a")
    return _text(balances, "balance", default="n/a")


def _draftable_candidate_count(snapshot: dict[str, Any]) -> int:
    candidates = _read(snapshot, "candidates", []) or []
    if candidates:
        return sum(1 for item in candidates if recommend_order_draft({"candidates": [_candidate_as_dict(item)]}) is not None)
    return int(_read(snapshot, "candidate_count", 0) or 0)


def _funnel_summary_line(snapshot: dict[str, Any], width: int) -> str:
    scan = _read(snapshot, "event_count", _read(snapshot, "scanned_event_count", 0)) or 0
    full_scan_limit = _read(snapshot, "scan_limit", scan) or scan
    window_limit = _read(snapshot, "window_limit", scan) or scan
    complete = _read(snapshot, "complete_event_count", 0) or 0
    edge = _read(snapshot, "candidate_count", 0) or 0
    draftable = _draftable_candidate_count(snapshot)
    selected = 1 if _read(snapshot, "order_draft", None) else 0
    return _fit(
        f"FUNNEL [{('windowed' if scan < (_read(snapshot, 'scan_limit', scan) or scan) else 'paged')}] scan={scan}/{_read(snapshot, 'scan_limit', scan) or scan} pages={_read(snapshot, 'page_count', 1) or 1} -> complete={complete} -> edge={edge} -> draft={draftable} -> selected={selected}",
        width,
    )


def _order_draft_line(snapshot: dict[str, Any], width: int) -> str | None:
    draft = _read(snapshot, "order_draft", {}) or {}
    if not isinstance(draft, dict):
        draft = getattr(draft, "__dict__", {}) or {}
    if not draft:
        return None
    return _fit(
        "draft={side} {size} @ {price} | {order_type} | token={token}".format(
            side=draft.get("side", "?"),
            size=draft.get("size", "?"),
            price=draft.get("price", "?"),
            order_type=draft.get("order_type", "GTC"),
            token=str(draft.get("token_id", ""))[:12],
        ),
        width,
    )


def _render_rich_lines(renderable, width: int) -> list[str]:
    sink = io.StringIO()
    console = Console(file=sink, width=max(20, width), record=True, force_terminal=False, color_system=None)
    console.print(renderable)
    return console.export_text(styles=False).splitlines()


def _main_dashboard_lines(snapshot: dict[str, Any], width: int, height: int, candidate_offset: int) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    pnl_summary = account_snapshot.get("pnl_summary", {}) if isinstance(account_snapshot, dict) else {}
    candidates = _candidate_items(snapshot)
    scan = _read(snapshot, "event_count", _read(snapshot, "scanned_event_count", 0)) or 0
    full_scan_limit = _read(snapshot, "scan_limit", scan) or scan
    window_limit = _read(snapshot, "window_limit", scan) or scan
    complete = _read(snapshot, "complete_event_count", 0) or 0
    edge = _read(snapshot, "candidate_count", 0) or 0
    draftable = _draftable_candidate_count(snapshot)
    selected = 1 if _read(snapshot, "order_draft", None) else 0
    target_interval = _read(snapshot, "target_interval_seconds", None)
    actual_cycle_ms = _read(snapshot, "actual_cycle_ms", None)
    scan_offset = int(_read(snapshot, "scan_offset", 0) or 0)
    scan_window_end = scan_offset + int(window_limit or scan or 0)
    reference_suffix = ""
    if str(_read(snapshot, "scan_mode", "full")).lower() == "hot":
        reference_suffix = "  refFull={count}@{edge}".format(
            count=_read(snapshot, "reference_candidate_count", 0),
            edge=_fmt_signed_num(_read(snapshot, "reference_best_gross_edge", 0.0), 3),
        )
    realtime_status = _text(snapshot, "realtime_status", default="unknown")
    realtime_reason = _text(snapshot, "realtime_reason", default="")
    overview_lines = [
        Text(
            "scan latency={lat} ms  target={target}  actualCycle={cycle}  mode={mode}  cadence={hot}/{full} every={every}  realtime={status}".format(
                lat=_read(snapshot, "scan_latency_ms", 0),
                target=f"{target_interval}s" if target_interval not in (None, "") else "n/a",
                cycle=f"{actual_cycle_ms}ms" if actual_cycle_ms not in (None, "") else "n/a",
                mode=str(_read(snapshot, "scan_mode", "full")).upper(),
                hot=_read(snapshot, "hot_limit", _read(snapshot, "scan_limit", 0)),
                full=full_scan_limit,
                every=_read(snapshot, "full_scan_every", 1),
                status=realtime_status,
            )
        )
    ]
    if realtime_reason:
        overview_lines.append(Text(f"reason={realtime_reason}"))
    header = Text(
        f"Polymarket Monitor [{'OK' if (_read(snapshot, 'source_status', _read(snapshot, 'status', 'unknown')) == 'ok') else 'WARN'}]  "
        f"iteration={_read(snapshot, 'iteration', '?')}  "
        f"data={_read(snapshot, 'data_source', 'unknown')}  "
        f"updated={_fmt_gmt8(_read(snapshot, 'updated_at', 'unknown'))}",
        style="bold",
    )

    overview = Group(
        *overview_lines,
        Text(
            f"FUNNEL [{'windowed' if window_limit < full_scan_limit else 'paged'}]  scan={scan}/{full_scan_limit}  "
            f"pages={_read(snapshot, 'page_count', 1) or 1}  window={scan_offset}:{scan_window_end}  hotspot={_read(snapshot, 'hotspot_offset', 0)}  "
            f"complete={complete}  edge={edge}  draft={draftable}  selected={selected}{reference_suffix}"
        ),
        Text(f"RULES {_rules_lines(snapshot, max(20, width - 2), 4)[2]}"),
        Text(_recommended_draft_line(snapshot, width) or _reference_draft_line(snapshot, width) or "BEST RULE COMBO  none"),
    )

    candidate_table = Table(box=box.SIMPLE_HEAVY, expand=True)
    candidate_table.add_column("rk", width=3)
    candidate_table.add_column("gross", width=8)
    candidate_table.add_column("net", width=8)
    candidate_table.add_column("title")
    if candidates:
        start = min(max(0, candidate_offset), max(0, len(candidates) - 1))
        visible = candidates[start : start + 8]
        for idx, item in enumerate(visible, start=start + 1):
            net = _read(item, "net_edge", _read(item, "adjusted_edge", _read(item, "cost_adjusted_edge")))
            marker = "▶" if idx == start + 1 else ""
            candidate_table.add_row(
                f"{marker}{idx}",
                _fmt_num(_read(item, "gross_edge"), 3),
                _fmt_num(net, 3),
                _text(item, "title", default="unknown"),
            )
    else:
        candidate_table.add_row("-", "-", "-", "No candidates")

    balances = account_snapshot.get("balances", {}) if isinstance(account_snapshot, dict) else {}
    buying_power = _text((balances.get("buyingPower") or {}) if isinstance(balances, dict) else {}, "value", default="n/a")
    open_order_funds = _text((balances.get("openOrders") or {}) if isinstance(balances, dict) else {}, "value", default="n/a")
    pnl_delta_value = _read(pnl_summary, "estimated_total_pnl_delta")
    equity_delta_value = _read(pnl_summary, "estimated_equity_delta")
    pnl_delta = f" Δ{_fmt_signed_num(pnl_delta_value, 2)}" if pnl_delta_value not in (None, "") else ""
    equity_delta = f" Δ{_fmt_signed_num(equity_delta_value, 2)}" if equity_delta_value not in (None, "") else ""
    account_group = Group(
        Text(f"mode={_read(_read(snapshot, 'account_status', {}) or {}, 'mode', 'unknown')}  sigType={_read(_read(snapshot, 'account_status', {}) or {}, 'signature_type', 'n/a')}  funder={'yes' if _read(_read(snapshot, 'account_status', {}) or {}, 'funder_present') else 'no'}"),
        Text(
            f"balance={_balance_value(balances)}  pnl={_fmt_num(_read(pnl_summary, 'estimated_total_pnl'), 2)}"
            f"{pnl_delta}  equity={_fmt_num(_read(pnl_summary, 'estimated_equity'), 2)}{equity_delta}"
        ),
        Text(f"buyingPower={buying_power}  openOrderFunds={open_order_funds}"),
        Text(
            f"positions={len(account_snapshot.get('positions', []) or [])}  recent_trades={len(_trades_list(account_snapshot))}"
            f"  fees={_fmt_num(_read(pnl_summary, 'fees_paid'), 2)}"
        ),
    )
    position_group = Group(*[Text(line) for line in _position_pnl_lines(snapshot, max(20, width // 2), 5, limit=5)])

    rules_group = Group(
        Text("FILTER CHAIN", style="bold"),
        Text("1. open markets only"),
        Text("2. supported structure"),
        Text("3. overlap/open-set rejection"),
        Text(f"4. gross_edge >= {_fmt_num(_read(snapshot, 'min_edge_threshold', 0.03), 3)}"),
        Text(
            "5. draft net_edge > 0  "
            f"(fee={_fmt_num(_read(snapshot, 'fee_buffer', 0.02), 3)} slip={_fmt_num(_read(snapshot, 'slippage_buffer', 0.005), 3)} safety={_fmt_num(_read(snapshot, 'safety_buffer', 0.005), 3)})"
            f" | hot={_read(snapshot, 'hot_limit', _read(snapshot, 'scan_limit', 0))} full={_read(snapshot, 'scan_limit', 0)} every={_read(snapshot, 'full_scan_every', 1)}"
        ),
    )

    focused = _focused_candidate_lines(snapshot, max(20, width - 6), 6, candidate_offset)
    focused_panel = Panel("\n".join(focused[1:]) if len(focused) > 1 else "none", title="FOCUSED CANDIDATE", box=box.ROUNDED)

    candidate_panel = Panel(candidate_table, title=f"CANDIDATE QUEUE ({len(candidates)})", box=box.ROUNDED)
    account_panel = Panel(account_group, title="ACCOUNT", box=box.ROUNDED)
    rules_panel = Panel(rules_group, title="RULES / THRESHOLDS", box=box.ROUNDED)
    position_panel = Panel(position_group, title=f"POSITION PNL ({len(account_snapshot.get('positions', []) or [])})", box=box.ROUNDED)
    reference_panel = Panel(_reference_group(snapshot), title="LAST FULL REFERENCE", box=box.ROUNDED)
    show_reference_panel = str(_read(snapshot, "scan_mode", "full")).lower() == "hot" or bool(_read(snapshot, "reference_order_draft", None)) or bool(_read(snapshot, "reference_candidate_count", 0))
    compact_side_panel = Panel(
        Group(
            Text("ACCOUNT", style="bold"),
            *list(account_group.renderables),
            Text(""),
            Text("RULES / THRESHOLDS", style="bold"),
            *list(rules_group.renderables),
            *(
                [Text(""), Text("LAST FULL REFERENCE", style="bold"), *list(_reference_group(snapshot).renderables)]
                if show_reference_panel
                else []
            ),
        ),
        title="ACCOUNT | RULES / THRESHOLDS",
        box=box.ROUNDED,
    )
    medium_side_panel = Panel(
        Group(
            Text(f"mode={_read(_read(snapshot, 'account_status', {}) or {}, 'mode', 'unknown')}  sigType={_read(_read(snapshot, 'account_status', {}) or {}, 'signature_type', 'n/a')}"),
            Text(
                f"balance={_balance_value(balances)}  pnl={_fmt_num(_read(pnl_summary, 'estimated_total_pnl'), 2)}"
                f"{pnl_delta}"
            ),
            Text("RULES / THRESHOLDS"),
            Text(f"gross>={_fmt_num(_read(snapshot, 'min_edge_threshold', 0.03), 3)} | draft net>0"),
            *([Text("LAST FULL REFERENCE"), *list(_reference_group(snapshot).renderables)] if show_reference_panel else []),
        ),
        title="ACCOUNT | RULES / THRESHOLDS",
        box=box.ROUNDED,
    )
    if height < 18:
        body = Group(candidate_panel, compact_side_panel)
        include_focused = False
    elif height < 24:
        body = Columns([candidate_panel, medium_side_panel], equal=False, expand=True)
        include_focused = True
    else:
        body = Columns(
            [
                candidate_panel,
                Group(
                    account_panel,
                    rules_panel,
                    *([reference_panel] if show_reference_panel else []),
                    position_panel,
                ),
            ],
            equal=False,
            expand=True,
        )
        include_focused = True
    footer = Text("controls: p submit | j/k scroll candidates | PgUp/PgDn jump | Ctrl-C exit")
    renderable_items = [header, Panel(overview, title="OVERVIEW", box=box.ROUNDED), body]
    if include_focused:
        renderable_items.append(focused_panel)
    renderable_items.append(footer)
    renderable = Group(*renderable_items)
    lines = _render_rich_lines(renderable, width)
    if len(lines) <= height:
        return lines
    if lines and lines[-1].startswith("controls:") and height >= 2:
        return lines[: height - 1] + [lines[-1]]
    return lines[:height]


def _account_dashboard_lines(
    snapshot: dict[str, Any],
    width: int,
    height: int,
    account_section: str,
    account_offsets: dict[str, int],
) -> list[str]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    balances = account_snapshot.get("balances", {}) if isinstance(account_snapshot, dict) else {}
    target_interval = _read(snapshot, "target_interval_seconds", None)
    actual_cycle_ms = _read(snapshot, "actual_cycle_ms", None)
    scan_offset = int(_read(snapshot, "scan_offset", 0) or 0)
    header = Text(
        f"Polymarket Account [{'OK' if (_read(snapshot, 'source_status', _read(snapshot, 'status', 'unknown')) == 'ok') else 'WARN'}]  "
        f"iteration={_read(snapshot, 'iteration', '?')}  "
        f"updated={_fmt_gmt8(_read(snapshot, 'updated_at', 'unknown'))}",
        style="bold",
    )
    overview = Group(
        Text(
            "scan latency={lat} ms  target={target}  actualCycle={cycle}  mode={mode}  balance={balance}".format(
                lat=_read(snapshot, "scan_latency_ms", 0),
                target=f"{target_interval}s" if target_interval not in (None, "") else "n/a",
                cycle=f"{actual_cycle_ms}ms" if actual_cycle_ms not in (None, "") else "n/a",
                mode=str(_read(snapshot, "scan_mode", "full")).upper(),
                balance=_balance_value(balances),
            )
        ),
        Text(
            f"allowances={len((balances.get('allowances') or {})) if isinstance(balances, dict) else 0}  "
            f"openOrders={_open_order_count(account_snapshot)}  "
            f"positions={len(account_snapshot.get('positions', []) or [])}  "
            f"recentTrades={len(_trades_list(account_snapshot))}  "
            f"window={scan_offset}:{scan_offset + int(_read(snapshot, 'event_count', 0) or 0)}"
        ),
        Text(
            f"activity={'empty' if (_open_order_count(account_snapshot)==0 and not (account_snapshot.get('positions') or []) and len(_trades_list(account_snapshot))==0) else 'present'}  "
            f"auth={_read(_read(snapshot, 'account_status', {}) or {}, 'mode', 'unknown')}"
        ),
    )
    draft_line = _order_draft_line(snapshot, width)
    result_line = _order_result_line(snapshot, width)
    if draft_line:
        overview.renderables.append(Text(draft_line))
    if result_line:
        overview.renderables.append(Text(result_line))

    status_panel = Panel("\n".join(_account_lines(snapshot, max(20, width // 2), 6)), title="ACCOUNT STATUS", box=box.ROUNDED)
    orders_panel = Panel(
        "\n".join(_open_order_lines_with_offset(snapshot, max(20, width // 2), 6, account_offsets.get("orders", 0))),
        title=_focused_title("OPEN ORDERS", len(_orders_list(account_snapshot)), account_section == "orders"),
        box=box.ROUNDED,
    )
    positions_panel = Panel(
        "\n".join(_positions_lines_with_offset(snapshot, max(20, width // 2), 6, account_offsets.get("positions", 0))),
        title=_focused_title("OPEN POSITIONS", len(account_snapshot.get("positions", []) or []), account_section == "positions"),
        box=box.ROUNDED,
    )
    trades_panel = Panel(
        "\n".join(_recent_trade_lines_with_offset(snapshot, max(20, width // 2), 6, account_offsets.get("trades", 0))),
        title=_focused_title("RECENT TRADES", len(_trades_list(account_snapshot)), account_section == "trades"),
        box=box.ROUNDED,
    )

    if width >= 110:
        body = Group(
            Columns([status_panel, orders_panel], expand=True, equal=True),
            Columns([positions_panel, trades_panel], expand=True, equal=True),
        )
    else:
        body = Group(status_panel, orders_panel, positions_panel, trades_panel)

    footer = Text("controls: Tab focus | j/k scroll | PgUp/PgDn jump | p submit | Ctrl-C exit")
    renderable = Group(header, Panel(overview, title="ACCOUNT OVERVIEW", box=box.ROUNDED), body, footer)
    lines = _render_rich_lines(renderable, width)
    if len(lines) <= height:
        return lines
    if lines and lines[-1].startswith("controls:") and height >= 2:
        return lines[: height - 1] + [lines[-1]]
    return lines[:height]


def _order_result_line(snapshot: dict[str, Any], width: int) -> str | None:
    result = _read(snapshot, "order_result", {}) or {}
    if not isinstance(result, dict):
        result = getattr(result, "__dict__", {}) or {}
    if not result:
        return None
    if result.get("success"):
        text = "submit=ok"
        if result.get("order_id"):
            text += f" | order={result['order_id']}"
        return _fit(text, width)
    return _fit(f"submit=error | {result.get('reason', 'unknown')}", width)


def _recommended_draft_line(snapshot: dict[str, Any], width: int) -> str | None:
    draft = _read(snapshot, "order_draft", {}) or {}
    if not isinstance(draft, dict):
        draft = getattr(draft, "__dict__", {}) or {}
    if not draft or draft.get("source") != "rule":
        return None
    return _fit(
        "BEST RULE COMBO | {title} | bundle={bundle} total={total} net={net}".format(
            title=draft.get("title", "candidate"),
            bundle=draft.get("bundle_size", "?"),
            total=_fmt_num(draft.get("total_price"), 3),
            net=_fmt_num(draft.get("net_edge"), 3),
        ),
        width,
    )


def _reference_draft_line(snapshot: dict[str, Any], width: int) -> str | None:
    draft = _read(snapshot, "reference_order_draft", {}) or {}
    if not isinstance(draft, dict):
        draft = getattr(draft, "__dict__", {}) or {}
    if not draft:
        return None
    return _fit(
        "LAST FULL COMBO | {title} | bundle={bundle} total={total} net={net}".format(
            title=draft.get("title", "candidate"),
            bundle=draft.get("bundle_size", "?"),
            total=_fmt_num(draft.get("total_price"), 3),
            net=_fmt_num(draft.get("net_edge"), 3),
        ),
        width,
    )


def _reference_group(snapshot: dict[str, Any]) -> Group:
    reference_line = _reference_draft_line(snapshot, 120) or "LAST FULL COMBO | none"
    return Group(
        Text(
            "refFull={count}@{edge}  hotspot={hotspot}".format(
                count=_read(snapshot, "reference_candidate_count", 0),
                edge=_fmt_signed_num(_read(snapshot, "reference_best_gross_edge", 0.0), 3),
                hotspot=_read(snapshot, "hotspot_offset", 0),
            )
        ),
        Text(reference_line),
    )


def _account_items(snapshot: dict[str, Any], section: str) -> list[dict[str, Any]]:
    account_snapshot = _read(snapshot, "account_snapshot", {}) or {}
    if section == "orders":
        return _orders_list(account_snapshot)
    if section == "positions":
        return account_snapshot.get("positions", []) or []
    if section == "trades":
        return _trades_list(account_snapshot)
    return []


def _handle_account_key(
    key: int,
    snapshot: dict[str, Any],
    focused_section: str,
    offsets: dict[str, int],
    page_step: int = 5,
) -> tuple[str, dict[str, int], dict[str, Any] | None]:
    sections = ("orders", "positions", "trades")
    section = focused_section if focused_section in sections else "orders"
    next_offsets = dict(offsets)
    action = None
    if key == ord("\t"):
        idx = sections.index(section)
        return sections[(idx + 1) % len(sections)], next_offsets, action
    if key == ord("p"):
        return section, next_offsets, {"type": "submit_order"}
    if key in (ord("j"), curses.KEY_DOWN):
        total = len(_account_items(snapshot, section))
        next_offsets[section] = min(next_offsets.get(section, 0) + 1, max(0, total - 1))
    elif key in (ord("k"), curses.KEY_UP):
        next_offsets[section] = max(0, next_offsets.get(section, 0) - 1)
    elif key == curses.KEY_NPAGE:
        total = len(_account_items(snapshot, section))
        next_offsets[section] = min(next_offsets.get(section, 0) + page_step, max(0, total - 1))
    elif key == curses.KEY_PPAGE:
        next_offsets[section] = max(0, next_offsets.get(section, 0) - page_step)
    return section, next_offsets, action


def _handle_monitor_key(
    key: int,
    snapshot: dict[str, Any],
    watched_offset: int,
    page_step: int = 5,
) -> tuple[int, dict[str, Any] | None]:
    action = None
    if key == ord("p") and _read(snapshot, "order_draft", None):
        return watched_offset, {"type": "submit_order"}
    watched_total = len(_candidate_items(snapshot) or (_read(snapshot, "watched_events", []) or []))
    if key in (ord("j"), curses.KEY_DOWN):
        watched_offset = min(watched_offset + 1, max(0, watched_total - 1))
    elif key in (ord("k"), curses.KEY_UP):
        watched_offset = max(0, watched_offset - 1)
    elif key == curses.KEY_NPAGE:
        watched_offset = min(watched_offset + page_step, max(0, watched_total - 1))
    elif key == curses.KEY_PPAGE:
        watched_offset = max(0, watched_offset - page_step)
    return watched_offset, action


def build_monitor_lines(
    snapshot: dict[str, Any],
    width: int = 120,
    height: int = 30,
    watched_offset: int = 0,
    account_section: str = "orders",
    account_offsets: dict[str, int] | None = None,
) -> list[str]:
    account_view = _read(snapshot, "view", "") == "account"
    if account_view:
        return _account_dashboard_lines(
            snapshot,
            width,
            height,
            account_section,
            account_offsets or {"orders": 0, "positions": 0, "trades": 0},
        )
    return _main_dashboard_lines(snapshot, width, height, watched_offset)


def render_snapshot_lines(
    snapshot: dict[str, Any],
    width: int = 120,
    height: int = 30,
    watched_offset: int = 0,
    account_section: str = "orders",
    account_offsets: dict[str, int] | None = None,
) -> list[str]:
    return build_monitor_lines(
        snapshot,
        width=width,
        height=height,
        watched_offset=watched_offset,
        account_section=account_section,
        account_offsets=account_offsets,
    )


class CursesMonitorView:
    def __init__(self) -> None:
        self.stdscr = curses.initscr()
        self.watched_offset = 0
        self.account_section = "orders"
        self.account_offsets = {"orders": 0, "positions": 0, "trades": 0}
        self.pending_action = None
        curses.noecho()
        curses.cbreak()
        self.stdscr.nodelay(True)
        self.stdscr.keypad(True)
        self._has_colors = False
        self._pair_header = 0
        self._pair_focus = 0
        self._pair_good = 0
        self._pair_warn = 0
        self._pair_dim = 0
        if hasattr(curses, "curs_set"):
            try:
                curses.curs_set(0)
            except curses.error:
                pass
        try:
            if curses.has_colors():
                curses.start_color()
                curses.use_default_colors()
                curses.init_pair(1, curses.COLOR_CYAN, -1)
                curses.init_pair(2, curses.COLOR_BLACK, curses.COLOR_CYAN)
                curses.init_pair(3, curses.COLOR_GREEN, -1)
                curses.init_pair(4, curses.COLOR_YELLOW, -1)
                curses.init_pair(5, curses.COLOR_BLUE, -1)
                self._has_colors = True
                self._pair_header = curses.color_pair(1) | curses.A_BOLD
                self._pair_focus = curses.color_pair(2) | curses.A_BOLD
                self._pair_good = curses.color_pair(3) | curses.A_BOLD
                self._pair_warn = curses.color_pair(4) | curses.A_BOLD
                self._pair_dim = curses.color_pair(5)
        except curses.error:
            self._has_colors = False

    def _line_attr(self, line: str):
        if not self._has_colors:
            return curses.A_NORMAL
        if line.startswith("Polymarket Monitor"):
            if "[OK]" in line:
                return self._pair_good
            return self._pair_warn
        if "BEST RULE COMBO" in line or "POSITION PNL" in line:
            return self._pair_header
        if " Δ+" in line or "▲ +" in line or "pnl=+" in line:
            return self._pair_good
        if " Δ-" in line or "▼ -" in line or "pnl=-" in line:
            return self._pair_warn
        if line.startswith("┌") or line.startswith("└") or line.startswith("│"):
            return self._pair_header if ("Overview" in line or "Top " in line or "Reject " in line or "WATCHED" in line or "STRATEGY" in line or "FOCUSED" in line) else curses.A_NORMAL
        if "▶" in line:
            return self._pair_focus
        if line.startswith("═") or line.startswith("─"):
            return self._pair_dim
        return curses.A_NORMAL

    def render(self, snapshot: dict[str, Any]) -> None:
        key = self.stdscr.getch()
        if _read(snapshot, "view", "") == "account":
            self.account_section, self.account_offsets, self.pending_action = _handle_account_key(
                key,
                snapshot,
                self.account_section,
                self.account_offsets,
            )
        else:
            self.watched_offset, self.pending_action = _handle_monitor_key(
                key,
                snapshot,
                self.watched_offset,
            )

        height, width = self.stdscr.getmaxyx()
        lines = build_monitor_lines(
            snapshot,
            width=width,
            height=height,
            watched_offset=self.watched_offset,
            account_section=self.account_section,
            account_offsets=self.account_offsets,
        )
        self.stdscr.erase()
        for index, line in enumerate(lines):
            try:
                self.stdscr.addnstr(index, 0, line, max(0, width - 1), self._line_attr(line))
            except curses.error:
                continue
        self.stdscr.refresh()
        return self.consume_action()

    def consume_action(self):
        action = self.pending_action
        self.pending_action = None
        return action

    def close(self) -> None:
        try:
            self.stdscr.keypad(False)
            curses.nocbreak()
            curses.echo()
            curses.endwin()
        except curses.error:
            pass
