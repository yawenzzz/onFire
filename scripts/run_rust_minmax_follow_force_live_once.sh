#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-}"

proxy_from_args() {
  local prev=""
  for arg in "$@"; do
    if [[ "$prev" == "--proxy" ]]; then
      printf '%s\n' "$arg"
      return 0
    fi
    prev="$arg"
  done
  return 1
}

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

CLI_PROXY="$(proxy_from_args "$@" || true)"
if [[ -n "${PROXY_DEFAULT}" && -z "${CLI_PROXY}" ]]; then
  args+=(--proxy "$PROXY_DEFAULT")
fi

echo "== rust minmax force live once =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
if [[ -n "${CLI_PROXY}" ]]; then
  echo "proxy=$CLI_PROXY"
elif [[ -n "${PROXY_DEFAULT}" ]]; then
  echo "proxy=$PROXY_DEFAULT"
else
  echo "proxy=disabled"
fi
echo

"$CARGO_BIN" "${args[@]}" "$@"
