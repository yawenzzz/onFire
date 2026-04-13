from __future__ import annotations

from decimal import Decimal, getcontext
from typing import Any

getcontext().prec = 40

D = Decimal
ZERO = D("0")
ONE = D("1")

CATEGORY_FEE_RATE: dict[str, Decimal] = {
    "crypto": D("0.072"),
    "sports": D("0.03"),
    "finance": D("0.04"),
    "politics": D("0.04"),
    "economics": D("0.05"),
    "culture": D("0.05"),
    "weather": D("0.05"),
    "other": D("0.05"),
    "general": D("0.05"),
    "mentions": D("0.04"),
    "tech": D("0.04"),
    "geopolitics": D("0"),
    "world": D("0"),
}


def _read(obj: Any, key: str, default=None):
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def normalize_category(category: str | None) -> str:
    if not category:
        return "other"
    value = category.strip().lower()
    aliases = {
        "other / general": "other",
        "general / other": "other",
        "world events": "world",
        "mention": "mentions",
        "technology": "tech",
        "political": "politics",
        "economic": "economics",
    }
    return aliases.get(value, value)


def fee_rate_for_candidate(candidate: Any) -> Decimal:
    if _read(candidate, "fees_enabled", True) is False:
        return ZERO
    category = normalize_category(_read(candidate, "category", None))
    return CATEGORY_FEE_RATE.get(category, CATEGORY_FEE_RATE["other"])


def buy_cost_for_net_shares(net_shares: Decimal, price: Decimal, fee_rate: Decimal) -> Decimal:
    denominator = ONE - fee_rate * (ONE - price)
    if denominator <= ZERO:
        return D("Infinity")
    gross_needed = net_shares / denominator
    return gross_needed * price


def fee_adjusted_bundle_cost(net_shares: Decimal, prices: list[Decimal], fee_rate: Decimal) -> Decimal:
    total = ZERO
    for price in prices:
        total += buy_cost_for_net_shares(net_shares, price, fee_rate)
    return total
