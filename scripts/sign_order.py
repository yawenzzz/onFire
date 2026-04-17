#!/usr/bin/env python3
"""Repo-local Polymarket order signing helper for Rust command bridges."""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any

try:
    from py_clob_client.config import get_contract_config
    from py_order_utils.builders.order_builder import OrderBuilder
    from py_order_utils.model import OrderData
    try:
        from py_order_utils.model.sides import BUY, SELL
    except Exception:  # pragma: no cover - brownfield SDK layout compatibility
        try:
            from py_order_utils.model import BUY, SELL
        except Exception:
            BUY, SELL = "BUY", "SELL"
    try:
        from py_order_utils.signer import Signer
    except Exception:  # pragma: no cover - brownfield SDK layout compatibility
        from py_clob_client.signer import Signer
except Exception as exc:  # pragma: no cover - local dependency opt-in
    print(f"py-clob-client / py-order-utils import failed: {exc}", file=sys.stderr)
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


def require_field(payload: dict[str, Any], name: str) -> Any:
    value = payload.get(name)
    if value is None or (isinstance(value, str) and not value.strip()):
        raise HelperError(f"missing payload field: {name}")
    return value


def as_int(value: Any, field: str) -> int:
    try:
        return int(str(value))
    except (TypeError, ValueError) as exc:
        raise HelperError(f"invalid integer for {field}: {value}") from exc


def as_side(value: Any) -> int:
    side = str(value).upper()
    if side == "BUY":
        return BUY
    if side == "SELL":
        return SELL
    raise HelperError(f"invalid side: {value}")


def build_order_builder(
    signer: Signer,
    signature_type: int,
    funder: str | None,
    contract_config: Any,
    chain_id: int,
) -> OrderBuilder:
    try:
        return OrderBuilder(contract_config.exchange, chain_id, signer)
    except TypeError:
        return OrderBuilder(signer, signature_type, funder, contract_config)


def build_signer(private_key: str, chain_id: int) -> Signer:
    try:
        return Signer(private_key, chain_id=chain_id)
    except TypeError:
        return Signer(private_key)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="read JSON payload from stdin")
    args = parser.parse_args()

    try:
        payload = load_payload(args.json)
        chain_id = as_int(env_value("CHAIN_ID", default="137"), "CHAIN_ID")
        private_key = require_env("PRIVATE_KEY", "CLOB_PRIVATE_KEY")
        signature_type = as_int(env_value("SIGNATURE_TYPE", default="0"), "SIGNATURE_TYPE")
        funder = env_value("FUNDER_ADDRESS", "FUNDER")
        signer = build_signer(private_key, chain_id)
        signer_address = env_value("POLY_ADDRESS", "SIGNER_ADDRESS", default=signer.address())
        maker = require_field(payload, "maker") if signature_type != 0 else str(payload.get("maker") or signer_address)
        contract_config = get_contract_config(chain_id, neg_risk=False)
        order_builder = build_order_builder(
            signer,
            signature_type,
            funder,
            contract_config,
            chain_id,
        )
        order_data = OrderData(
            maker=str(maker),
            taker=str(require_field(payload, "taker")),
            tokenId=as_int(require_field(payload, "tokenId"), "tokenId"),
            makerAmount=as_int(require_field(payload, "makerAmount"), "makerAmount"),
            takerAmount=as_int(require_field(payload, "takerAmount"), "takerAmount"),
            side=as_side(require_field(payload, "side")),
            feeRateBps=str(as_int(require_field(payload, "feeRateBps"), "feeRateBps")),
            nonce=str(as_int(require_field(payload, "nonce"), "nonce")),
            signer=str(payload.get("signer") or signer_address),
            expiration=str(as_int(require_field(payload, "expiration"), "expiration")),
            signatureType=signature_type,
        )
        signed = order_builder.build_signed_order(order_data)
        order = signed.dict() if hasattr(signed, "dict") else {}
        output = {
            "signature": str(order.get("signature") or getattr(signed, "signature", "")),
            "salt": str(order.get("salt") or getattr(signed, "salt", "")),
            "order": {
                **({k: v for k, v in order.items() if k not in {"signature", "salt"}} if isinstance(order, dict) else {}),
                "maker": str(order.get("maker") or getattr(signed, "maker", order_data.maker)),
                "signer": str(order.get("signer") or getattr(signed, "signer", order_data.signer)),
            },
        }
        if not output["signature"] or not output["salt"]:
            raise HelperError("signed order helper returned empty signature or salt")
        print(json.dumps(output))
        return 0
    except HelperError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except Exception as exc:  # pragma: no cover - real local SDK execution
        print(f"order signing failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
