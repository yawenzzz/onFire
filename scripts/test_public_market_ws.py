#!/usr/bin/env python3
from __future__ import annotations

import argparse
import asyncio
import json
import sys

try:
    from websockets import connect
except Exception as exc:  # pragma: no cover - runtime convenience path
    raise SystemExit(f"websockets import failed: {exc}")


DEFAULT_WS_URL = "wss://ws-subscriptions-clob.polymarket.com/ws/market"


async def run_probe(ws_url: str, asset_ids: list[str], message_limit: int, timeout_seconds: float) -> int:
    async with connect(
        ws_url,
        proxy=None,
        open_timeout=timeout_seconds,
        ping_interval=20,
        ping_timeout=20,
    ) as ws:
        sub = {
            "assets_ids": asset_ids,
            "type": "market",
            "custom_feature_enabled": True,
        }
        await ws.send(json.dumps(sub))
        print("sub sent:", json.dumps(sub, ensure_ascii=False))

        count = 0
        while count < message_limit:
            msg = await asyncio.wait_for(ws.recv(), timeout=timeout_seconds)
            print(f"msg[{count}]={msg}")
            count += 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ws-url", default=DEFAULT_WS_URL)
    parser.add_argument("--asset-id", dest="asset_ids", action="append", required=True)
    parser.add_argument("--limit", type=int, default=3)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    args = parser.parse_args(argv)

    try:
        return asyncio.run(
            run_probe(
                ws_url=args.ws_url,
                asset_ids=args.asset_ids,
                message_limit=args.limit,
                timeout_seconds=args.timeout_seconds,
            )
        )
    except Exception as exc:  # pragma: no cover - runtime convenience path
        print(f"probe failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
