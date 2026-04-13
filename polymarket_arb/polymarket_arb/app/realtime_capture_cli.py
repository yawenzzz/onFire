from __future__ import annotations

import argparse
import asyncio

from polymarket_arb.app.capture_daemon import run_capture_daemon_once
from polymarket_arb.venue.default_real_ws_client import build_default_ws_client


def main(argv: list[str] | None = None, optional_factory_resolver=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--limit', type=int, default=1)
    parser.add_argument('--ws-url', required=True)
    args = parser.parse_args(argv)

    client = build_default_ws_client(args.ws_url, optional_factory_resolver)
    if client is None:
        return 2
    asyncio.run(run_capture_daemon_once(client, args.output, args.limit))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
