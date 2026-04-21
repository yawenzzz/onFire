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
  --loop-count
  1
  --allow-live-submit
  --force-live-submit
  --ignore-seen-tx
  --min-open-usdc
  1
  --max-open-usdc
  1
)

if [[ -n "${PROXY_DEFAULT}" ]]; then
  args+=(--proxy "$PROXY_DEFAULT")
fi

echo "== rust minmax force live once =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
echo "proxy=${PROXY_DEFAULT:-disabled}"
echo

"$CARGO_BIN" "${args[@]}" "$@"
