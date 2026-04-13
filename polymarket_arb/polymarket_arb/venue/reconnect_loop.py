from __future__ import annotations

import asyncio

from polymarket_arb.venue.backoff_policy import compute_backoff_seconds


async def run_reconnect_loop_once(runner, max_attempts: int, sleep_impl=None):
    if sleep_impl is None:
        sleep_impl = asyncio.sleep
    attempt = 0
    while attempt < max_attempts:
        attempt += 1
        try:
            return await runner()
        except Exception:
            if attempt >= max_attempts:
                raise
            await sleep_impl(compute_backoff_seconds(attempt))
