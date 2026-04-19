#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"

cd "$ROOT/rust-copytrader"

args=(
  run
  --bin
  run_copytrader_minmax_follow
  --
  --root
  ..
)

if [[ -n "${PROXY_DEFAULT}" ]]; then
  args+=(--proxy "$PROXY_DEFAULT")
fi

args+=("$@")

echo "== rust minmax follow =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
if [[ -n "${PROXY_DEFAULT}" ]]; then
  echo "proxy=$PROXY_DEFAULT"
else
  echo "proxy=disabled"
fi
echo

"$CARGO_BIN" "${args[@]}"
