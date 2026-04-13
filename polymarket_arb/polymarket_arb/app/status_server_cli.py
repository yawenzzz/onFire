from __future__ import annotations

import argparse
import time

from polymarket_arb.service.status_service import start_status_service


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', required=True)
    parser.add_argument('--host', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--once', action='store_true')
    args = parser.parse_args(argv)

    server = start_status_service(root=args.root, host=args.host, port=args.port)
    try:
        if args.once:
            return 0
        while True:
            time.sleep(1)
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    raise SystemExit(main())
