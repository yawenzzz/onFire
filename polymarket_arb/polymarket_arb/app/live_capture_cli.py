from __future__ import annotations

import argparse
import asyncio

from polymarket_arb.app.capture_daemon import run_capture_daemon_once
from polymarket_arb.auth.ws_auth import build_ws_auth_headers
from polymarket_arb.venue.default_real_ws_client import build_default_ws_client


class _DemoWSClient:
    async def iter_messages(self, limit: int):
        yield {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        if limit > 1:
            yield {'market_id': 'm2', 'market_state': 'OPEN', 'best_bid': 0.3, 'best_ask': 0.6}


def main(
    argv: list[str] | None = None,
    connect_factory=None,
    optional_factory_resolver=None,
    ws_factory_resolver=None,
) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--limit', type=int, default=1)
    parser.add_argument('--ws-url', required=True)
    parser.add_argument('--access-key')
    parser.add_argument('--signature')
    parser.add_argument('--timestamp')
    args = parser.parse_args(argv)

    headers = None
    if args.access_key and args.signature and args.timestamp:
        headers = build_ws_auth_headers(args.access_key, args.signature, args.timestamp)

    resolver = optional_factory_resolver if optional_factory_resolver is not None else ws_factory_resolver

    actual_factory = None
    if connect_factory is not None:
        try:
            actual_factory = connect_factory()
        except TypeError:
            actual_factory = connect_factory
    elif resolver is not None:
        actual_factory = resolver()

    if headers is not None and actual_factory is not None:
        original_factory = actual_factory
        actual_factory = lambda url: original_factory(url, headers=headers)

    client = build_default_ws_client(args.ws_url, lambda: actual_factory)
    if client is None:
        return 2
    asyncio.run(run_capture_daemon_once(client, args.output, args.limit))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
