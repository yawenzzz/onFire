from __future__ import annotations

import time
import json
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from datetime import datetime, timezone
import asyncio
import queue
import threading
from polymarket_arb.data.contracts import MarketSnapshot

from polymarket_arb.monitoring.gamma_client import GammaMarketClient
from polymarket_arb.monitoring.models import EventScanResult, MonitorSnapshot, ScanResult
from polymarket_arb.monitoring.rules import classify_complete_bucket_event
from polymarket_arb.data.market_message_normalizer import normalize_market_message
from polymarket_arb.venue.default_real_ws_client import build_default_ws_client
from polymarket_arb.venue.live_market_ws_adapter import LiveMarketWSAdapter
from polymarket_arb.venue.optional_ws_lib import websocket_connect_factory
from polymarket_arb.venue.subscription_planner import chunk_market_ids
from polymarket_arb.venue.ws_subscription import build_market_subscription_message
from polymarket_arb.venue.ws_transport_probe import probe_ws_transport

_CATEGORY_PRIORITY = [
    "Politics",
    "World",
    "Geopolitics",
    "Crypto",
    "Finance",
    "Economy",
    "Business",
    "Tech",
    "Sports",
    "Culture",
    "Entertainment",
]


@dataclass
class MonitorSettings:
    limit: int = 500
    min_edge: float = 0.01
    depth_window: int = 1
    hot_limit: int = 200
    full_scan_every: int = 5
    fee_buffer: float = 0.02
    slippage_buffer: float = 0.005
    safety_buffer: float = 0.005

    @property
    def total_cost_buffer(self) -> float:
        return self.fee_buffer + self.slippage_buffer + self.safety_buffer


class HighFrequencyScanner:
    def __init__(self, client=None, settings: MonitorSettings | None = None) -> None:
        self.client = client or GammaMarketClient()
        self.settings = settings or MonitorSettings()
        self.iteration = 0

    def scan_once(self) -> MonitorSnapshot:
        self.iteration += 1
        return scan_events_snapshot(self.client, settings=self.settings, iteration=self.iteration)


class _RealtimeMarketStream:
    def __init__(self, ws_client) -> None:
        self.ws_client = ws_client
        self._adapter = LiveMarketWSAdapter()
        self._queue: queue.Queue = queue.Queue()
        self._thread = None
        self._started = False
        self.last_error: str | None = None
        self.connected = False
        self.asset_ids: list[str] = []

    def set_asset_ids(self, asset_ids: list[str]) -> None:
        self.asset_ids = list(dict.fromkeys([str(x) for x in asset_ids if x]))

    def start(self) -> None:
        if self._started or self.ws_client is None or hasattr(self.ws_client, "drain_messages"):
            return
        self._started = True

        def _runner():
            async def _consume():
                source = self.ws_client.connect_impl(self.ws_client.url)
                async with source as conn:
                    for chunk in chunk_market_ids(self.asset_ids, 200) if self.asset_ids else []:
                        await conn.send(json.dumps(build_market_subscription_message(list(chunk))))
                    while True:
                        try:
                            raw = await conn.recv()
                        except StopAsyncIteration:
                            break
                        try:
                            self.connected = True
                            for parsed in _parse_realtime_messages(raw):
                                self._queue.put(parsed)
                        except Exception:
                            continue
            try:
                asyncio.run(_consume())
            except Exception as exc:
                self.last_error = f"{type(exc).__name__}: {exc}"

        self._thread = threading.Thread(target=_runner, daemon=True)
        self._thread.start()

    def drain(self) -> list:
        if self.ws_client is not None and hasattr(self.ws_client, "drain_messages"):
            out = []
            for raw in self.ws_client.drain_messages():
                try:
                    self.connected = True
                    for parsed in _parse_realtime_messages(raw):
                        out.append(parsed)
                except Exception:
                    continue
            return out
        out = []
        while True:
            try:
                out.append(self._queue.get_nowait())
            except queue.Empty:
                return out


def _best_bid_from_levels(levels: list[dict]) -> float:
    prices = [_safe_float(level.get("price")) for level in levels if _safe_float(level.get("price")) is not None]
    return max(prices) if prices else 0.0


def _best_ask_from_levels(levels: list[dict]) -> float:
    prices = [_safe_float(level.get("price")) for level in levels if _safe_float(level.get("price")) is not None]
    return min(prices) if prices else 1.0


def _parse_realtime_messages(raw) -> list[MarketSnapshot]:
    if isinstance(raw, dict) and {"market_id", "market_state", "best_bid", "best_ask"} <= set(raw.keys()):
        return [normalize_market_message(raw)]
    payload = json.loads(raw) if isinstance(raw, str) else raw
    if not isinstance(payload, dict):
        return []
    event_type = payload.get("event_type")
    if event_type == "book":
        asset_id = payload.get("asset_id")
        if not asset_id:
            return []
        bids = payload.get("bids") or []
        asks = payload.get("asks") or []
        return [
            MarketSnapshot(
                market_id=str(asset_id),
                market_state="OPEN",
                best_bid=_best_bid_from_levels(bids),
                best_ask=_best_ask_from_levels(asks),
            )
        ]
    if event_type == "price_change":
        snaps: list[MarketSnapshot] = []
        for item in payload.get("price_changes") or []:
            asset_id = item.get("asset_id")
            if not asset_id:
                continue
            snaps.append(
                MarketSnapshot(
                    market_id=str(asset_id),
                    market_state="OPEN",
                    best_bid=float(item.get("best_bid") or 0.0),
                    best_ask=float(item.get("best_ask") or 1.0),
                )
            )
        return snaps
    return []


def _event_category(event: dict) -> str:
    if event.get("category"):
        return str(event["category"])
    labels: list[str] = []
    for tag in event.get("tags", []) or []:
        if isinstance(tag, dict):
            label = tag.get("label")
            if label:
                labels.append(str(label))
        elif tag:
            labels.append(str(tag))
    lowered = {label.lower(): label for label in labels}
    for category in _CATEGORY_PRIORITY:
        if category.lower() in lowered:
            return lowered[category.lower()]
    return "Other"


def _safe_float(value) -> float | None:
    if value in (None, ""):
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def parse_jsonish(value):
    if value is None:
        return None
    if isinstance(value, (list, dict)):
        return value
    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return value
    return value


def _first_open_market(event: dict) -> dict | None:
    for market in event.get("markets", []) or []:
        if market.get("active") and not market.get("closed"):
            return market
    return None


def _first_token_id(market: dict | None) -> str | None:
    if not market:
        return None
    raw = market.get("clobTokenIds")
    if raw in (None, ""):
        return None
    if isinstance(raw, list):
        return str(raw[0]) if raw else None
    if isinstance(raw, str):
        try:
            values = json.loads(raw)
            return str(values[0]) if values else None
        except json.JSONDecodeError:
            return None
    return None


def _market_token_id(market: dict | None) -> str | None:
    return _first_token_id(market)


def _all_token_ids(market: dict | None) -> list[str]:
    if not market:
        return []
    raw = market.get("clobTokenIds")
    parsed = parse_jsonish(raw)
    if isinstance(parsed, list):
        return [str(item) for item in parsed if item not in (None, "")]
    if isinstance(market.get("tokens"), list):
        out = []
        for token in market.get("tokens") or []:
            if not isinstance(token, dict):
                continue
            token_id = token.get("token_id") or token.get("id")
            if token_id:
                out.append(str(token_id))
        return out
    return []


def _recommended_orders(event: dict) -> list[dict]:
    orders = []
    open_markets = [
        market
        for market in event.get("markets", []) or []
        if market.get("active") and not market.get("closed") and market.get("bestAsk") is not None
    ]
    sizes = []
    for market in open_markets:
        minimum = _safe_float(market.get("orderMinSize")) or 1.0
        sizes.append(minimum)
    bundle_size = max(sizes) if sizes else 1.0
    for market in open_markets:
        token_id = _market_token_id(market)
        if not token_id:
            continue
        orders.append(
            {
                "token_id": token_id,
                "title": market.get("question") or event.get("title", ""),
                "side": "BUY",
                "price": _safe_float(market.get("bestAsk")),
                "size": bundle_size,
                "order_type": "GTC",
                "tick_size": _safe_float(market.get("orderPriceMinTickSize")),
                "order_min_size": _safe_float(market.get("orderMinSize")),
            }
        )
    return orders


def _book_depth(book: dict | None) -> tuple[float | None, float | None, float | None, float | None]:
    if not book:
        return None, None, None, None
    bids = book.get("bids") or []
    asks = book.get("asks") or []
    bid_top = _safe_float((bids[0] if bids else {}).get("size")) if bids else None
    ask_top = _safe_float((asks[0] if asks else {}).get("size")) if asks else None
    bid_5 = sum(_safe_float(level.get("size")) or 0.0 for level in bids[:5]) if bids else None
    ask_5 = sum(_safe_float(level.get("size")) or 0.0 for level in asks[:5]) if asks else None
    return bid_top, ask_top, bid_5, ask_5


def _result_from_event(event: dict, min_edge: float) -> EventScanResult:
    classification = classify_complete_bucket_event(event)
    questions = [str(market.get("question", "")) for market in event.get("markets", [])]
    market = _first_open_market(event)
    token_id = _first_token_id(market)
    open_market_count = len(
        [market for market in event.get("markets", []) if market.get("active") and not market.get("closed")]
    )
    total_market_count = len(event.get("markets", []))
    sum_ask = round(1.0 - classification.gross_edge, 6)
    rejection_reason = classification.rejection_reason or (classification.reasons[0] if classification.reasons else None)
    return EventScanResult(
        slug=event.get("slug", ""),
        title=event.get("title", ""),
        template_type=classification.structure_type,
        is_complete=classification.is_complete,
        open_market_count=open_market_count,
        total_market_count=total_market_count,
        sum_ask=sum_ask,
        gross_edge=classification.gross_edge,
        cost_adjusted_edge=classification.gross_edge - min_edge,
        rejection_reason=rejection_reason,
        category=_event_category(event),
        event_url=f"https://polymarket.com/event/{event.get('slug', '')}",
        resolution_source=event.get("resolutionSource") or None,
        market_question=(market or {}).get("question"),
        best_bid=_safe_float((market or {}).get("bestBid")),
        best_ask=_safe_float((market or {}).get("bestAsk")),
        last_trade_price=_safe_float((market or {}).get("lastTradePrice")),
        spread=_safe_float((market or {}).get("spread")),
        volume_24hr=_safe_float((market or {}).get("volume24hr") or event.get("volume24hr")),
        liquidity=_safe_float((market or {}).get("liquidity") or event.get("liquidity")),
        order_min_size=_safe_float((market or {}).get("orderMinSize")),
        tick_size=_safe_float((market or {}).get("orderPriceMinTickSize")),
        depth_bid_top=None,
        depth_ask_top=None,
        depth_bid_5=None,
        depth_ask_5=None,
        clob_token_id=token_id,
        questions=questions,
        recommended_orders=_recommended_orders(event),
    )


def _attach_depth(result: EventScanResult, client) -> None:
    token_id = result.clob_token_id
    if not token_id:
        return
    try:
        book = client.fetch_book(token_id)
    except Exception:
        return
    depth_bid_top, depth_ask_top, depth_bid_5, depth_ask_5 = _book_depth(book)
    result.depth_bid_top = depth_bid_top
    result.depth_ask_top = depth_ask_top
    result.depth_bid_5 = depth_bid_5
    result.depth_ask_5 = depth_ask_5


def _market_identifier(event: dict, market: dict, index: int) -> str | None:
    token_ids = _all_token_ids(market)
    if token_ids:
        return token_ids[0]
    return str(
        market.get("slug")
        or market.get("marketSlug")
        or market.get("id")
        or market.get("market_id")
        or market.get("marketId")
        or f"{event.get('slug', 'event')}:{index}"
    )


def _snapshot_from_events(
    events: list[dict],
    client,
    limit: int,
    min_edge: float,
    depth_window: int,
    start_offset: int,
    fee_buffer: float,
    slippage_buffer: float,
    safety_buffer: float,
    data_source: str = "gamma-api",
    repeat_intervals: dict[str, int] | None = None,
) -> MonitorSnapshot:
    repeat_intervals = repeat_intervals or {}
    started = time.time()
    result = scan_events(events, min_edge=min_edge)
    for item in result.candidates + result.rejections:
        item.repeat_interval_ms = repeat_intervals.get(item.slug)
    complete_event_count = len(result.candidates) + sum(1 for item in result.rejections if item.is_complete)
    rejection_counts: dict[str, int] = {}
    category_counts: dict[str, int] = {}
    structure_counts: dict[str, int] = {}
    for item in result.rejections:
        reason = item.rejection_reason or "unknown"
        rejection_counts[reason] = rejection_counts.get(reason, 0) + 1
        category = item.category or "unknown"
        category_counts[category] = category_counts.get(category, 0) + 1
        structure = item.template_type or "unsupported_structure"
        structure_counts[structure] = structure_counts.get(structure, 0) + 1
    for item in result.candidates:
        category = item.category or "unknown"
        category_counts[category] = category_counts.get(category, 0) + 1
        structure = item.template_type or "complete_bucket"
        structure_counts[structure] = structure_counts.get(structure, 0) + 1
    complete_results = result.candidates + [item for item in result.rejections if item.is_complete]
    best_gross_edge = max((item.gross_edge for item in complete_results), default=0.0)
    best_cost_adjusted_edge = max((item.cost_adjusted_edge for item in result.candidates), default=0.0)
    hotspot_offset = start_offset
    if result.candidates:
        best_slug = result.candidates[0].slug
        for index, event in enumerate(events):
            if event.get("slug", "") == best_slug:
                hotspot_offset = start_offset + index
                break
    watched_events = [
        {
            "title": item.title,
            "category": item.category or "unknown",
            "status": "PASS" if item in result.candidates else "REJECT",
            "structure": item.template_type or "unsupported_structure",
            "reason": item.rejection_reason or (item.template_type or "complete_bucket"),
            "best_bid": item.best_bid,
            "best_ask": item.best_ask,
            "last_trade_price": item.last_trade_price,
            "spread": item.spread,
            "volume_24hr": item.volume_24hr,
            "liquidity": item.liquidity,
            "depth_bid_top": item.depth_bid_top,
            "depth_ask_top": item.depth_ask_top,
            "event_url": item.event_url,
            "resolution_source": item.resolution_source,
            "repeat_interval_ms": item.repeat_interval_ms,
        }
        for item in (result.candidates + result.rejections)
    ]
    for item in (result.candidates + result.rejections)[: max(0, depth_window)]:
        _attach_depth(item, client)
    watched_events = [
        {
            "title": item.title,
            "category": item.category or "unknown",
            "status": "PASS" if item in result.candidates else "REJECT",
            "structure": item.template_type or "unsupported_structure",
            "reason": item.rejection_reason or (item.template_type or "complete_bucket"),
            "best_bid": item.best_bid,
            "best_ask": item.best_ask,
            "last_trade_price": item.last_trade_price,
            "spread": item.spread,
            "volume_24hr": item.volume_24hr,
            "liquidity": item.liquidity,
            "depth_bid_top": item.depth_bid_top,
            "depth_ask_top": item.depth_ask_top,
            "event_url": item.event_url,
            "resolution_source": item.resolution_source,
            "repeat_interval_ms": item.repeat_interval_ms,
        }
        for item in (result.candidates + result.rejections)
    ]
    return MonitorSnapshot(
        iteration=1,
        event_count=len(events),
        complete_event_count=complete_event_count,
        candidate_count=len(result.candidates),
        best_gross_edge=best_gross_edge,
        best_cost_adjusted_edge=best_cost_adjusted_edge,
        scan_duration_seconds=time.time() - started,
        candidates=result.candidates,
        rejections=result.rejections,
        rejection_counts=rejection_counts,
        category_counts=category_counts,
        structure_counts=structure_counts,
        watched_events=watched_events,
        data_source=data_source,
        status="ok",
        updated_at=datetime.now(timezone.utc).isoformat(),
        scanned_event_count=len(events),
        page_count=max(1, (len(events) + 99) // 100),
        scan_limit=limit,
        window_limit=limit,
        scan_offset=start_offset,
        hotspot_offset=hotspot_offset,
        min_edge_threshold=min_edge,
        fee_buffer=fee_buffer,
        slippage_buffer=slippage_buffer,
        safety_buffer=safety_buffer,
    )


def scan_events(events: list[dict], min_edge: float) -> ScanResult:
    candidates: list[EventScanResult] = []
    rejections: list[EventScanResult] = []
    for event in events:
        result = _result_from_event(event, min_edge=min_edge)
        if result.is_complete and result.gross_edge >= min_edge:
            candidates.append(result)
        else:
            rejections.append(result)
    candidates.sort(key=lambda item: item.gross_edge, reverse=True)
    return ScanResult(candidates=candidates, rejections=rejections)


def _fetch_event_window(
    client,
    limit: int,
    closed: str = "false",
    page_size: int = 100,
    max_workers: int = 4,
    start_offset: int = 0,
) -> tuple[list[dict], int]:
    if limit <= 0:
        return [], 0
    page_requests = [
        (start_offset + offset, min(page_size, limit - offset))
        for offset in range(0, limit, page_size)
    ]
    if len(page_requests) == 1:
        offset, batch_limit = page_requests[0]
        batch = client.fetch_events(limit=batch_limit, closed=closed, offset=offset)
        return list(batch or []), 1

    fetched_pages: dict[int, tuple[int, list[dict]]] = {}
    with ThreadPoolExecutor(max_workers=min(max_workers, len(page_requests))) as executor:
        futures = {
            executor.submit(client.fetch_events, limit=batch_limit, closed=closed, offset=offset): (offset, batch_limit)
            for offset, batch_limit in page_requests
        }
        for future, (offset, batch_limit) in futures.items():
            fetched_pages[offset] = (batch_limit, list(future.result() or []))

    events: list[dict] = []
    pages = 0
    for offset, batch_limit in page_requests:
        expected_limit, batch = fetched_pages[offset]
        pages += 1
        events.extend(batch)
        if len(batch) < expected_limit:
            break
    return events[:limit], pages


def scan_complete_bucket_events(
    client,
    limit: int = 500,
    min_edge: float = 0.03,
    depth_window: int = 5,
    start_offset: int = 0,
    fee_buffer: float = 0.02,
    slippage_buffer: float = 0.005,
    safety_buffer: float = 0.005,
) -> MonitorSnapshot:
    events, pages = _fetch_event_window(client, limit=limit, closed="false", start_offset=start_offset)
    snapshot = _snapshot_from_events(
        events,
        client=client,
        limit=limit,
        min_edge=min_edge,
        depth_window=depth_window,
        start_offset=start_offset,
        fee_buffer=fee_buffer,
        slippage_buffer=slippage_buffer,
        safety_buffer=safety_buffer,
        data_source="gamma-api",
    )
    snapshot.page_count = pages
    return snapshot


def _hot_window_offset(settings: MonitorSettings, hot_iteration_index: int, preferred_offset: int | None = None) -> int:
    hot_window = max(1, min(settings.hot_limit, settings.limit))
    window_offsets = list(range(0, settings.limit, hot_window)) or [0]
    if preferred_offset is not None:
        preferred_window = min((preferred_offset // hot_window) * hot_window, window_offsets[-1])
        ordered_offsets = [preferred_window] + [offset for offset in window_offsets if offset != preferred_window]
    else:
        ordered_offsets = window_offsets
    return ordered_offsets[hot_iteration_index % len(ordered_offsets)]


def scan_events_snapshot(client, settings: MonitorSettings, iteration: int = 1, preferred_hot_offset: int | None = None) -> MonitorSnapshot:
    is_full_scan = iteration == 1 or ((iteration - 1) % max(1, settings.full_scan_every) == 0)
    scan_limit = settings.limit if is_full_scan else min(settings.hot_limit, settings.limit)
    if is_full_scan:
        scan_offset = 0
    else:
        hot_iteration_index = iteration - 2
        scan_offset = _hot_window_offset(settings, hot_iteration_index, preferred_offset=preferred_hot_offset)
    snapshot = scan_complete_bucket_events(
        client,
        limit=scan_limit,
        min_edge=settings.min_edge,
        depth_window=settings.depth_window,
        start_offset=scan_offset,
        fee_buffer=settings.fee_buffer,
        slippage_buffer=settings.slippage_buffer,
        safety_buffer=settings.safety_buffer,
    )
    snapshot.iteration = iteration
    snapshot.top_candidates = snapshot.candidates[:5]
    snapshot.scanned_event_count = snapshot.event_count
    snapshot.scan_latency_ms = int(snapshot.scan_duration_seconds * 1000)
    snapshot.scan_mode = "full" if is_full_scan else "hot"
    snapshot.hot_limit = min(settings.hot_limit, settings.limit)
    snapshot.full_scan_every = settings.full_scan_every
    hot_window = max(1, min(settings.hot_limit, settings.limit))
    snapshot.hot_window_count = max(1, (settings.limit + hot_window - 1) // hot_window)
    snapshot.window_limit = scan_limit
    snapshot.scan_limit = settings.limit
    snapshot.scan_offset = scan_offset
    snapshot.source_ok = True
    snapshot.source_message = "ok"
    return snapshot


class GammaScanner:
    def __init__(
        self,
        client,
        min_edge_threshold: float = 0.01,
        limit: int = 500,
        depth_window: int = 1,
        hot_limit: int = 200,
        full_scan_every: int = 5,
    ) -> None:
        self.client = client
        self.min_edge_threshold = min_edge_threshold
        self.limit = limit
        self.depth_window = depth_window
        self.hot_limit = hot_limit
        self.full_scan_every = full_scan_every
        self.iteration = 0
        self.last_full_snapshot: MonitorSnapshot | None = None
        self.covered_hot_offsets: set[int] = set()

    def scan(self, limit: int = 50, iteration: int = 1) -> MonitorSnapshot:
        preferred_hot_offset = self.last_full_snapshot.hotspot_offset if self.last_full_snapshot is not None else None
        return scan_events_snapshot(
            self.client,
            settings=MonitorSettings(
                limit=limit,
                min_edge=self.min_edge_threshold,
                depth_window=self.depth_window,
                hot_limit=min(self.hot_limit, limit),
                full_scan_every=self.full_scan_every,
            ),
            iteration=iteration,
            preferred_hot_offset=preferred_hot_offset,
        )

    def scan_once(self) -> MonitorSnapshot:
        self.iteration += 1
        snapshot = self.scan(limit=self.limit, iteration=self.iteration)
        if snapshot.scan_mode == "full":
            self.last_full_snapshot = snapshot
            self.covered_hot_offsets = {0}
            snapshot.covered_window_count = snapshot.hot_window_count
        elif self.last_full_snapshot is not None:
            snapshot.reference_candidate_count = self.last_full_snapshot.candidate_count
            snapshot.reference_best_gross_edge = self.last_full_snapshot.best_gross_edge
            self.covered_hot_offsets.add(snapshot.scan_offset)
            snapshot.covered_window_count = len(self.covered_hot_offsets)
        return snapshot


class RealtimeGammaScanner:
    def __init__(
        self,
        client=None,
        min_edge_threshold: float = 0.01,
        limit: int = 500,
        depth_window: int = 1,
        hot_limit: int = 200,
        full_scan_every: int = 5,
        ws_client=None,
        transport_probe=None,
    ) -> None:
        self.client = client or GammaMarketClient()
        self.settings = MonitorSettings(
            limit=limit,
            min_edge=min_edge_threshold,
            depth_window=depth_window,
            hot_limit=hot_limit,
            full_scan_every=full_scan_every,
        )
        self.iteration = 0
        self._events: list[dict] | None = None
        self._market_index: dict[str, tuple[int, int]] = {}
        self._event_repeat_ms: dict[str, int] = {}
        self._event_last_seen_at: dict[str, float] = {}
        self.last_full_snapshot: MonitorSnapshot | None = None
        self.covered_hot_offsets: set[int] = set()
        self.reference_order_draft = None
        self._ws_setup_error: str | None = None
        self._transport_probe = transport_probe or probe_ws_transport
        self._stream = _RealtimeMarketStream(ws_client or self._build_default_ws_client())

    def _build_default_ws_client(self):
        ws_url = "wss://ws-subscriptions-clob.polymarket.com/ws/market"
        ok, reason = self._transport_probe(ws_url)
        if not ok:
            self._ws_setup_error = reason
            return None
        return build_default_ws_client(
            ws_url,
            optional_factory_resolver=websocket_connect_factory,
        )

    def _bootstrap(self) -> bool:
        if self._events is not None:
            return False
        events, _pages = _fetch_event_window(self.client, limit=self.settings.limit, closed="false", start_offset=0)
        self._events = events
        self._market_index = {}
        asset_ids: list[str] = []
        for event_index, event in enumerate(self._events):
            for market_index, market in enumerate(event.get("markets", []) or []):
                token_ids = _all_token_ids(market)
                if token_ids:
                    for token_id in token_ids:
                        self._market_index[str(token_id)] = (event_index, market_index)
                        asset_ids.append(str(token_id))
                    continue
                market_id = _market_identifier(event, market, market_index)
                if market_id:
                    self._market_index[str(market_id)] = (event_index, market_index)
        self._stream.set_asset_ids(asset_ids)
        self._stream.start()
        return True

    def _apply_updates(self, updates: list) -> None:
        if not updates or self._events is None:
            return
        now_ms = int(time.time() * 1000)
        for update in updates:
            market_id = str(getattr(update, "market_id", ""))
            location = self._market_index.get(market_id)
            if location is None:
                continue
            event_index, market_index = location
            event = self._events[event_index]
            market = (event.get("markets") or [])[market_index]
            market["bestBid"] = getattr(update, "best_bid", market.get("bestBid"))
            market["bestAsk"] = getattr(update, "best_ask", market.get("bestAsk"))
            state = getattr(update, "market_state", market.get("market_state", "OPEN"))
            market["active"] = state == "OPEN"
            market["closed"] = state != "OPEN"
            slug = str(event.get("slug", ""))
            previous = self._event_last_seen_at.get(slug)
            if previous is not None:
                self._event_repeat_ms[slug] = now_ms - int(previous)
            self._event_last_seen_at[slug] = now_ms

    def scan_once(self) -> MonitorSnapshot:
        bootstrapped_now = self._bootstrap()
        assert self._events is not None
        self.iteration += 1
        updates = [] if bootstrapped_now else self._stream.drain()
        self._apply_updates(updates)
        is_full_scan = self.iteration == 1 or ((self.iteration - 1) % max(1, self.settings.full_scan_every) == 0)
        if is_full_scan:
            scan_offset = 0
            window_limit = self.settings.limit
            window_events = self._events
        else:
            preferred_hot_offset = self.last_full_snapshot.hotspot_offset if self.last_full_snapshot is not None else None
            hot_iteration_index = self.iteration - 2
            scan_offset = _hot_window_offset(self.settings, hot_iteration_index, preferred_offset=preferred_hot_offset)
            window_limit = min(self.settings.hot_limit, self.settings.limit)
            window_events = self._events[scan_offset: scan_offset + window_limit]
        snapshot = _snapshot_from_events(
            window_events,
            client=self.client,
            limit=self.settings.limit,
            min_edge=self.settings.min_edge,
            depth_window=self.settings.depth_window,
            start_offset=scan_offset,
            fee_buffer=self.settings.fee_buffer,
            slippage_buffer=self.settings.slippage_buffer,
            safety_buffer=self.settings.safety_buffer,
            data_source="gamma-ws",
            repeat_intervals=self._event_repeat_ms,
        )
        snapshot.iteration = self.iteration
        snapshot.scan_mode = "full" if is_full_scan else "hot"
        snapshot.hot_limit = min(self.settings.hot_limit, self.settings.limit)
        snapshot.full_scan_every = self.settings.full_scan_every
        snapshot.scan_limit = self.settings.limit
        snapshot.window_limit = window_limit
        snapshot.hot_window_count = max(1, (self.settings.limit + snapshot.hot_limit - 1) // snapshot.hot_limit)
        if self._ws_setup_error:
            snapshot.realtime_status = "error"
            snapshot.realtime_reason = self._ws_setup_error
        elif self._stream.last_error:
            snapshot.realtime_status = "error"
            snapshot.realtime_reason = self._stream.last_error
        elif self._stream.connected:
            snapshot.realtime_status = "connected"
            snapshot.realtime_reason = "ws updates flowing"
        else:
            snapshot.realtime_status = "connecting"
            snapshot.realtime_reason = "awaiting ws messages"
        if snapshot.scan_mode == "full":
            self.last_full_snapshot = snapshot
            self.covered_hot_offsets = {0}
            snapshot.covered_window_count = snapshot.hot_window_count
        elif self.last_full_snapshot is not None:
            snapshot.reference_candidate_count = self.last_full_snapshot.candidate_count
            snapshot.reference_best_gross_edge = self.last_full_snapshot.best_gross_edge
            self.covered_hot_offsets.add(snapshot.scan_offset)
            snapshot.covered_window_count = len(self.covered_hot_offsets)
        return snapshot
