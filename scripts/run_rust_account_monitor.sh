#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
INTERVAL_SECS="${INTERVAL_SECS:-5}"
OUTPUT_PATH="${OUTPUT_PATH:-.omx/account-monitor/latest.json}"

args=("$@")
if [[ $# -eq 0 ]]; then
  args=(--root "$ROOT")
elif [[ " ${args[*]-} " != *" --root "* ]]; then
  args=(--root "$ROOT" "${args[@]}")
fi

cd "$ROOT/rust-copytrader"
"$CARGO_BIN" run --bin run_copytrader_account_monitor -- --watch --json --interval-secs "$INTERVAL_SECS" --output "$OUTPUT_PATH" "${args[@]}"
