#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${PYTHON_BIN:-python3}"
CARGO_BIN="${CARGO_BIN:-cargo}"
export RUST_COPYTRADER_SIGNING_PROGRAM="${RUST_COPYTRADER_SIGNING_PROGRAM:-$PYTHON_BIN}"
export RUST_COPYTRADER_SUBMIT_PROGRAM="${RUST_COPYTRADER_SUBMIT_PROGRAM:-$PYTHON_BIN}"
export RUST_COPYTRADER_SUBMIT_ARGS="${RUST_COPYTRADER_SUBMIT_ARGS:-scripts/submit_helper.py --json --curl-bin curl}"

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "missing required env: ${name}" >&2
    exit 2
  fi
}

if [[ -z "${POLY_ADDRESS:-}" && -z "${SIGNER_ADDRESS:-}" ]]; then
  echo "missing required env: POLY_ADDRESS" >&2
  exit 2
fi

require_env "CLOB_API_KEY"
require_env "CLOB_SECRET"
require_env "CLOB_PASS_PHRASE"

if [[ -z "${PRIVATE_KEY:-}" && -z "${CLOB_PRIVATE_KEY:-}" ]]; then
  echo "missing required env: PRIVATE_KEY" >&2
  exit 2
fi

SIGNATURE_TYPE="${SIGNATURE_TYPE:-0}"
if [[ "${SIGNATURE_TYPE}" != "0" && -z "${FUNDER_ADDRESS:-${FUNDER:-}}" ]]; then
  echo "missing required env: FUNDER_ADDRESS" >&2
  exit 2
fi

SIGNER_ADDR="${POLY_ADDRESS:-${SIGNER_ADDRESS:-}}"
MAKER_ADDR="${FUNDER_ADDRESS:-${FUNDER:-$SIGNER_ADDR}}"

cd "$ROOT"

echo "== rust helper smoke =="
echo "root=$ROOT"
echo "python=$PYTHON_BIN"
echo "cargo=$CARGO_BIN"
echo "signer=$SIGNER_ADDR"
echo "signature_type=$SIGNATURE_TYPE"
echo

echo "== sign_order.py =="
printf '%s\n' \
  "{\"maker\":\"$MAKER_ADDR\",\"signer\":\"$SIGNER_ADDR\",\"signatureType\":$SIGNATURE_TYPE,\"taker\":\"0x0000000000000000000000000000000000000000\",\"tokenId\":\"12345\",\"makerAmount\":\"1000000\",\"takerAmount\":\"2000000\",\"side\":\"BUY\",\"expiration\":\"1735689600\",\"nonce\":\"7\",\"feeRateBps\":\"30\"}" \
  | "$PYTHON_BIN" scripts/sign_order.py --json
echo

echo "== sign_l2.py =="
printf '%s\n' \
  "{\"address\":\"$SIGNER_ADDR\",\"method\":\"POST\",\"requestPath\":\"/orders\",\"body\":\"{\\\"owner\\\":\\\"owner-uuid\\\"}\"}" \
  | "$PYTHON_BIN" scripts/sign_l2.py --json
echo

echo "== rust helper smoke report =="
( cd rust-copytrader && "$CARGO_BIN" run -- --smoke-helper --root .. )
