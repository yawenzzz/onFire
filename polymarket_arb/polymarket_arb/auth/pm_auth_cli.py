from __future__ import annotations

import argparse

from polymarket_arb.auth.pm_auth_generator import generate_pm_auth_exports


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--access-key", required=True)
    parser.add_argument("--private-key", required=True)
    parser.add_argument("--path", default="/v1/ws/markets")
    parser.add_argument("--method", default="GET")
    parser.add_argument("--timestamp")
    args = parser.parse_args(argv)

    exports = generate_pm_auth_exports(
        access_key=args.access_key,
        private_key_base64=args.private_key,
        path=args.path,
        method=args.method,
        timestamp_ms=args.timestamp,
    )
    for key, value in exports.items():
        print(f"{key}={value}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
