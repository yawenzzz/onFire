from __future__ import annotations

import argparse

from polymarket_arb.app.capture_bundle import run_capture_bundle


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--capture-file', required=True)
    parser.add_argument('--archive-root', required=True)
    parser.add_argument('--session-id', required=True)
    parser.add_argument('--surface-id', required=True)
    args = parser.parse_args(argv)

    run_capture_bundle(
        capture_path=args.capture_file,
        archive_root=args.archive_root,
        session_id=args.session_id,
        surface_id=args.surface_id,
    )
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
