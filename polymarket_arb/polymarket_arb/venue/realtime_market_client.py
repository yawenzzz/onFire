from __future__ import annotations

import json

from polymarket_arb.venue.ws_subscription import build_market_subscription_message


async def capture_market_messages(connect_factory, url: str, market_ids: list[str], limit: int):
    out = []
    async with connect_factory(url) as conn:
        await conn.send(json.dumps(build_market_subscription_message(market_ids)))
        while len(out) < limit:
            try:
                msg = await conn.recv()
            except StopAsyncIteration:
                break
            out.append(msg)
    return out
