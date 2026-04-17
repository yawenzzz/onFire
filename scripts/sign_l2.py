#!/usr/bin/env python3
"""Repo-local Polymarket L2 header signing helper for Rust command bridges."""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any

try:
    from py_clob_client.clob_types import ApiCreds, RequestArgs
    from py_clob_client.headers.headers import create_level_2_headers
    from py_clob_client.signer import Signer
except Exception as exc:  # pragma: no cover - local dependency opt-in
    print(f"py-clob-client import failed: {exc}", file=sys.stderr)
    raise SystemExit(1)


class HelperError(Exception):
    pass


def env_value(*keys: str, default: str | None = None) -> str | None:
    for key in keys:
        value = os.environ.get(key)
        if value is not None and value.strip():
            return value.strip()
    return default


def require_env(*keys: str) -> str:
    value = env_value(*keys)
    if value is None:
        raise HelperError(f"missing env: {keys[0]}")
    return value


def load_payload(expect_json: bool) -> dict[str, Any]:
    if not expect_json:
        raise HelperError("only --json stdin payloads are supported")
    raw = sys.stdin.read().strip()
    if not raw:
        raise HelperError("missing JSON payload on stdin")
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise HelperError(f"invalid JSON payload: {exc}") from exc
    if not isinstance(payload, dict):
        raise HelperError("expected JSON object payload")
    return payload


def require_field(payload: dict[str, Any], name: str) -> str:
    value = payload.get(name)
    if value is None or (isinstance(value, str) and not value.strip()):
        raise HelperError(f"missing payload field: {name}")
    return str(value)


def as_int(value: str | None, field: str) -> int:
    try:
        return int(str(value))
    except (TypeError, ValueError) as exc:
        raise HelperError(f"invalid integer for {field}: {value}") from exc


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="read JSON payload from stdin")
    args = parser.parse_args()

    try:
        payload = load_payload(args.json)
        chain_id = as_int(env_value("CHAIN_ID", default="137"), "CHAIN_ID")
        private_key = require_env("PRIVATE_KEY", "CLOB_PRIVATE_KEY")
        api_key = require_env("CLOB_API_KEY", "POLY_API_KEY")
        api_secret = require_env("CLOB_SECRET", "POLY_API_SECRET")
        passphrase = require_env("CLOB_PASS_PHRASE", "POLY_PASSPHRASE")
        signer = Signer(private_key, chain_id=chain_id)
        creds = ApiCreds(api_key=api_key, api_secret=api_secret, api_passphrase=passphrase)
        request_args = RequestArgs(
            method=require_field(payload, "method"),
            request_path=require_field(payload, "requestPath"),
            body=require_field(payload, "body"),
        )
        headers = create_level_2_headers(signer, creds, request_args)
        if not isinstance(headers, dict):
            raise HelperError("level2 helper did not return a header map")
        signature = headers.get("POLY_SIGNATURE") or headers.get("signature")
        timestamp = headers.get("POLY_TIMESTAMP") or headers.get("timestamp")
        if signature is None or timestamp is None:
            raise HelperError("level2 helper returned incomplete signature output")
        output = dict(headers)
        output["signature"] = str(signature)
        output["timestamp"] = str(timestamp)
        print(json.dumps(output))
        return 0
    except HelperError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except Exception as exc:  # pragma: no cover - real local SDK execution
        print(f"l2 header signing failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
