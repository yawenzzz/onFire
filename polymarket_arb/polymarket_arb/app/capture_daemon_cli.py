from __future__ import annotations

import argparse
import asyncio

from polymarket_arb.app.capture_daemon import run_capture_daemon_once
from polymarket_arb.venue.real_capture_factory import build_async_ws_client


class _DemoWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        if limit > 1:
            yield {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6}


def main(argv: list[str] | None = None, connect_factory=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--limit', type=int, default=1)
    parser.add_argument('--demo', action='store_true')
    parser.add_argument('--ws-url')
    args = parser.parse_args(argv)

    if args.demo:
        ws_client = _DemoWSClient()
    elif args.ws_url:
        actual_factory = None
        if connect_factory is not None:
            try:
                actual_factory = connect_factory()
            except TypeError:
                actual_factory = connect_factory
        ws_client = build_async_ws_client(args.ws_url, actual_factory)
        if ws_client is None:
            return 2
    else:
        return 2

    asyncio.run(run_capture_daemon_once(ws_client, args.output, args.limit))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
