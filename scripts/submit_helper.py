#!/usr/bin/env python3
"""Repo-local wrapper for curl-compatible submit commands.

This helper keeps the Rust command contract repo-local by accepting helper flags
first and forwarding the remaining curl-style arguments unchanged.
"""

from __future__ import annotations

import argparse
import subprocess
import sys


def parse_args(argv: list[str]) -> tuple[argparse.Namespace, list[str]]:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        action="store_true",
        help="enable the repo-local helper contract; forwarded arguments stay curl-compatible",
    )
    parser.add_argument(
        "--curl-bin",
        default="curl",
        help="curl-compatible binary to execute (defaults to curl)",
    )
    return parser.parse_known_args(argv)


def main(argv: list[str]) -> int:
    args, curl_args = parse_args(argv)
    if not curl_args:
        print("submit_helper.py expected curl-style arguments to forward", file=sys.stderr)
        return 2

    completed = subprocess.run(
        [args.curl_bin, *curl_args],
        capture_output=True,
        check=False,
    )
    sys.stdout.buffer.write(completed.stdout)
    sys.stderr.buffer.write(completed.stderr)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
