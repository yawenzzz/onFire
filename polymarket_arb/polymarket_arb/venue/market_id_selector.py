from __future__ import annotations


def select_market_ids_from_events(payload: dict, limit: int) -> list[str]:
    out: list[str] = []
    for event in payload.get('events', []):
        for market in event.get('markets', []):
            slug = market.get('slug') or market.get('marketSlug')
            if slug:
                out.append(slug)
                if len(out) >= limit:
                    return out
    return out
