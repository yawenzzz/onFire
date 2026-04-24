#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
INTERVAL_SECS="${INTERVAL_SECS:-30}"
LOG_DIR="${LOG_DIR:-$ROOT/logs/account-sweeper}"
LOG_FILE="${LOG_FILE:-$LOG_DIR/account-sweeper.log}"
ALLOW_LIVE_SUBMIT="${ALLOW_LIVE_SUBMIT:-1}"

mkdir -p "$LOG_DIR"

args=(--watch --interval-secs "$INTERVAL_SECS" "$@")
if [[ $# -eq 0 ]]; then
  args=(--watch --interval-secs "$INTERVAL_SECS" --root "$ROOT")
elif [[ " ${args[*]-} " != *" --root "* ]]; then
  args=(--watch --interval-secs "$INTERVAL_SECS" --root "$ROOT" "$@")
fi

if [[ "$ALLOW_LIVE_SUBMIT" != "0" && " ${args[*]-} " != *" --allow-live-submit "* ]]; then
  args+=(--allow-live-submit)
fi

printf '[info]: account sweeper independent loop\n'
printf '[info]: mode=%s\n' "$([[ "$ALLOW_LIVE_SUBMIT" != "0" ]] && printf 'live' || printf 'preview')"
printf '[info]: log_file=%s\n' "$LOG_FILE"
printf '[info]: independent_of_main_follow=true\n'

cd "$ROOT/rust-copytrader"
"$CARGO_BIN" run --bin run_copytrader_account_sweeper -- "${args[@]}" 2>&1 | tee -a "$LOG_FILE"
