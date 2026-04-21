#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"

args=("$@")
if [[ $# -eq 0 ]]; then
  args=(--root "$ROOT")
elif [[ " ${args[*]-} " != *" --root "* ]]; then
  args=(--root "$ROOT" "${args[@]}")
fi

cd "$ROOT/rust-copytrader"
"$CARGO_BIN" run --bin run_copytrader_account_monitor -- --json "${args[@]}"
