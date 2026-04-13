from __future__ import annotations

import argparse
import asyncio
import json
from pathlib import Path

from polymarket_arb.auth.env import load_ws_auth_config_from_env
from polymarket_arb.auth.ws_auth import build_ws_auth_headers
from polymarket_arb.ops.file_reporter import write_json_report
from polymarket_arb.venue.optional_ws_lib import websocket_connect_factory
from polymarket_arb.venue.realtime_market_client import capture_market_messages


def main(argv: list[str] | None = None, connect_factory=None, optional_factory_resolver=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--ws-url', required=True)
    parser.add_argument('--market-ids', required=True)
    parser.add_argument('--limit', type=int, default=1)
    parser.add_argument('--output', required=True)
    parser.add_argument('--access-key')
    parser.add_argument('--signature')
    parser.add_argument('--timestamp')
    args = parser.parse_args(argv)

    if connect_factory is None:
        connect_factory = optional_factory_resolver() if optional_factory_resolver is not None else None
    if connect_factory is None:
        connect_factory = optional_factory_resolver() if optional_factory_resolver is not None else websocket_connect_factory()
    if connect_factory is None:
        return 2

    headers = None
    if args.access_key and args.signature and args.timestamp:
        headers = build_ws_auth_headers(args.access_key, args.signature, args.timestamp)
    else:
        env_cfg = load_ws_auth_config_from_env()
        if env_cfg is not None and env_cfg.is_complete():
            headers = build_ws_auth_headers(env_cfg.access_key, env_cfg.signature, env_cfg.timestamp)

    async def _run():
        wrapped_factory = connect_factory
        if headers is not None:
            wrapped_factory = lambda url: connect_factory(url, headers=headers)
        msgs = await capture_market_messages(wrapped_factory, args.ws_url, args.market_ids.split(','), args.limit)
        write_json_report(args.output, {'captured_count': len(msgs), 'messages': msgs})

    asyncio.run(_run())
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
