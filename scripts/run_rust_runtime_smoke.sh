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

cd "$ROOT/rust-copytrader"
"$CARGO_BIN" run -- --smoke-runtime --root ..
