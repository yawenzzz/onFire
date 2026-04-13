from __future__ import annotations

from polymarket_arb.monitoring.models import ClassificationResult


def _open_markets(event: dict) -> list[dict]:
    return [
        market
        for market in event.get("markets", [])
        if market.get("active") and not market.get("closed") and market.get("bestAsk") is not None
    ]


def _questions(markets: list[dict]) -> list[str]:
    return [str(market.get("question", "")) for market in markets]


def _sum_ask(markets: list[dict]) -> float:
    return round(sum(float(market["bestAsk"]) for market in markets), 6)


def _build_result(
    event: dict,
    markets: list[dict],
    is_complete: bool,
    template_type: str | None,
    rejection_reason: str | None,
    gross_edge: float,
) -> ClassificationResult:
    normalized_type = template_type
    reasons: list[str] = []
    if rejection_reason:
        if rejection_reason.startswith("overlapping"):
            reasons = ["overlap", rejection_reason]
        elif rejection_reason == "open_candidate_set":
            reasons = ["open_set", rejection_reason]
        else:
            reasons = [rejection_reason]
    return ClassificationResult(
        slug=str(event.get("slug", "")),
        title=str(event.get("title", "")),
        is_complete=is_complete,
        template_type=normalized_type,
        structure_type=normalized_type,
        rejection_reason=rejection_reason,
        reasons=reasons,
        gross_edge=gross_edge,
        open_market_count=len(markets),
        total_market_count=int(
            event.get("eventMetadata", {}).get("market_count_total", len(event.get("markets", [])))
        ),
        open_sum_ask=_sum_ask(markets),
        questions=_questions(markets),
    )


def _is_market_cap_range_set(questions: list[str]) -> bool:
    text = " || ".join(questions).lower()
    return "market cap" in text and "not ipo by" in text and ("between $" in text or "less than $" in text)


def _is_count_bucket_set(title: str, questions: list[str]) -> bool:
    text = " || ".join(questions).lower()
    return "how many" in title.lower() and ("or more" in text or "no " in text)


def _has_overlapping_thresholds(questions: list[str]) -> bool:
    text = " || ".join(questions).lower()
    return "above $" in text and "between $" not in text and "or greater" not in text and "less than $" not in text


def _has_overlapping_sentence_ranges(questions: list[str]) -> bool:
    text = " || ".join(questions).lower()
    return "no prison time" in text and "less than" in text


def _is_open_candidate_set(event: dict, markets: list[dict]) -> bool:
    title = str(event.get("title", "")).lower()
    questions = _questions(markets)
    total_market_count = int(
        event.get("eventMetadata", {}).get("market_count_total", len(event.get("markets", [])))
    )
    if ("winner" in title or "prize" in title) and len(markets) < total_market_count:
        return True
    if ("winner" in title or "prize" in title) and not any("other" in question.lower() for question in questions):
        return True
    return False


def classify_complete_bucket_event(event: dict, min_edge: float = 0.0) -> ClassificationResult:
    markets = _open_markets(event)
    questions = _questions(markets)
    gross_edge = round(1.0 - _sum_ask(markets), 6)

    if _has_overlapping_thresholds(questions):
        return _build_result(event, markets, False, None, "overlapping_thresholds", gross_edge)

    if _has_overlapping_sentence_ranges(questions):
        return _build_result(event, markets, False, None, "overlapping_ranges", gross_edge)

    if _is_open_candidate_set(event, markets):
        return _build_result(event, markets, False, None, "open_candidate_set", gross_edge)

    if _is_market_cap_range_set(questions):
        result = _build_result(event, markets, True, "market_cap_buckets", None, gross_edge)
    elif _is_count_bucket_set(str(event.get("title", "")), questions):
        result = _build_result(event, markets, True, "count_buckets", None, gross_edge)
    else:
        return _build_result(event, markets, False, None, "unsupported_structure", gross_edge)

    if gross_edge < min_edge:
        result.rejection_reason = "edge_below_threshold"
        result.reasons = ["edge_below_threshold"]
    return result


def classify_event(event: dict, min_edge: float = 0.0) -> ClassificationResult:
    result = classify_complete_bucket_event(event, min_edge=min_edge)
    if result.structure_type == "market_cap_buckets":
        result.structure_type = "complete_bucket"
    elif result.structure_type == "count_buckets":
        result.structure_type = "count_bucket"
    return result
