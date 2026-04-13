from __future__ import annotations

import argparse

from polymarket_arb.app.capture_supervisor import run_capture_supervisor_once


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', required=True)
    parser.add_argument('--limit', type=int, default=1)
    args = parser.parse_args(argv)
    run_capture_supervisor_once(root=args.root, limit=args.limit)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
