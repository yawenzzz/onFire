#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
MONITOR_COLUMNS="${MONITOR_COLUMNS:-${COLUMNS:-140}}"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"

cd "$ROOT/rust-copytrader"

args=(
  run
  --bin
  run_copytrader_monitor_v1
  --
  --root
  ..
)

if [[ -n "${PROXY_DEFAULT}" ]]; then
  args+=(--proxy "$PROXY_DEFAULT")
fi

args+=("$@")

echo "== rust monitor v2 =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
echo "columns=$MONITOR_COLUMNS"
if [[ -n "${PROXY_DEFAULT}" ]]; then
  echo "proxy=$PROXY_DEFAULT"
else
  echo "proxy=disabled"
fi
echo

COLUMNS="$MONITOR_COLUMNS" "$CARGO_BIN" "${args[@]}"
