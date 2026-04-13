from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class ClassificationResult:
    slug: str
    title: str
    is_complete: bool
    template_type: str | None = None
    structure_type: str | None = None
    rejection_reason: str | None = None
    reasons: list[str] = field(default_factory=list)
    gross_edge: float = 0.0
    open_market_count: int = 0
    total_market_count: int = 0
    open_sum_ask: float = 0.0
    questions: list[str] = field(default_factory=list)

    @property
    def sum_ask(self) -> float:
        return self.open_sum_ask

    @property
    def passes_structure(self) -> bool:
        return self.is_complete

    @property
    def reject_reasons(self) -> list[str]:
        reasons = list(self.reasons)
        if not reasons and self.rejection_reason:
            reasons = [self.rejection_reason]
        aliases: list[str] = []
        for reason in reasons:
            if "overlap" in reason:
                aliases.append("overlap")
            if reason == "open_candidate_set":
                aliases.append("open_set")
        return reasons + [alias for alias in aliases if alias not in reasons]


RuleClassification = ClassificationResult


@dataclass
class RuleEngineResult:
    is_candidate: bool
    template_type: str | None = None
    rejection_reason: str | None = None
    gross_edge: float = 0.0


@dataclass
class EventScanResult:
    slug: str
    title: str
    template_type: str | None
    is_complete: bool
    open_market_count: int
    total_market_count: int
    sum_ask: float
    gross_edge: float
    cost_adjusted_edge: float
    rejection_reason: str | None
    category: str | None = None
    event_url: str | None = None
    resolution_source: str | None = None
    market_question: str | None = None
    best_bid: float | None = None
    best_ask: float | None = None
    last_trade_price: float | None = None
    spread: float | None = None
    volume_24hr: float | None = None
    liquidity: float | None = None
    order_min_size: float | None = None
    tick_size: float | None = None
    depth_bid_top: float | None = None
    depth_ask_top: float | None = None
    depth_bid_5: float | None = None
    depth_ask_5: float | None = None
    clob_token_id: str | None = None
    repeat_interval_ms: int | None = None
    questions: list[str] = field(default_factory=list)
    recommended_orders: list[dict] = field(default_factory=list)

    @property
    def reasons(self) -> list[str]:
        return [self.rejection_reason] if self.rejection_reason else []

    @property
    def reject_reasons(self) -> list[str]:
        reasons = self.reasons
        aliases: list[str] = []
        for reason in reasons:
            if "overlap" in reason:
                aliases.append("overlap")
            if reason == "open_candidate_set":
                aliases.append("open_set")
        return reasons + [alias for alias in aliases if alias not in reasons]


CandidateView = EventScanResult


@dataclass
class ScanResult:
    candidates: list[EventScanResult]
    rejections: list[EventScanResult]


@dataclass
class MonitorSnapshot:
    iteration: int
    event_count: int
    complete_event_count: int
    candidate_count: int
    best_gross_edge: float
    best_cost_adjusted_edge: float
    scan_duration_seconds: float
    candidates: list[EventScanResult] = field(default_factory=list)
    rejections: list[EventScanResult] = field(default_factory=list)
    rejection_counts: dict[str, int] = field(default_factory=dict)
    category_counts: dict[str, int] = field(default_factory=dict)
    structure_counts: dict[str, int] = field(default_factory=dict)
    watched_events: list[dict[str, str]] = field(default_factory=list)
    data_source: str = "gamma-api"
    status: str = "ok"
    updated_at: str | None = None
    top_candidates: list[EventScanResult] = field(default_factory=list)
    scanned_event_count: int = 0
    scan_latency_ms: int = 0
    page_count: int = 1
    scan_limit: int = 0
    window_limit: int = 0
    scan_offset: int = 0
    hotspot_offset: int = 0
    scan_mode: str = "full"
    hot_limit: int = 0
    full_scan_every: int = 1
    hot_window_count: int = 0
    covered_window_count: int = 0
    realtime_status: str = "unknown"
    realtime_reason: str = ""
    reference_candidate_count: int = 0
    reference_best_gross_edge: float = 0.0
    min_edge_threshold: float = 0.03
    fee_buffer: float = 0.02
    slippage_buffer: float = 0.005
    safety_buffer: float = 0.005
    source_ok: bool = True
    source_message: str = "ok"

    @property
    def source_status(self) -> str:
        return self.status

    @property
    def best_adjusted_edge(self) -> float:
        return self.best_cost_adjusted_edge

    @property
    def rejection_count(self) -> int:
        return len(self.rejections)

    @property
    def rejection_reason_counts(self) -> dict[str, int]:
        return self.rejection_counts

    def __post_init__(self) -> None:
        if not self.scanned_event_count:
            self.scanned_event_count = self.event_count
        if not self.top_candidates:
            self.top_candidates = self.candidates
        if not self.scan_latency_ms:
            self.scan_latency_ms = int(self.scan_duration_seconds * 1000)
        if not self.scan_limit:
            self.scan_limit = self.event_count
        if not self.window_limit:
            self.window_limit = self.event_count
        if not self.hot_limit:
            self.hot_limit = self.scan_limit
        if not self.hot_window_count:
            hot_window = max(1, min(self.hot_limit, self.scan_limit))
            self.hot_window_count = max(1, (self.scan_limit + hot_window - 1) // hot_window)
        if not self.covered_window_count:
            self.covered_window_count = self.hot_window_count if self.scan_mode == "full" else 1
        if self.status != "ok":
            self.source_ok = False
            if self.source_message == "ok":
                self.source_message = self.status
