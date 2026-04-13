from __future__ import annotations

import json

from polymarket_arb.data.market_message_normalizer import normalize_market_message


class LiveMarketWSAdapter:
    def parse_message(self, raw: str):
        parsed = json.loads(raw)
        if not isinstance(parsed, dict):
            return None
        return normalize_market_message(parsed)
