#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-}"
WATCH_BIN_DEFAULT="${WATCH_BIN_DEFAULT:-$ROOT/scripts/run_rust_watch_copy_leader_activity.sh}"
LIVE_SUBMIT_BIN_DEFAULT="${LIVE_SUBMIT_BIN_DEFAULT:-$ROOT/scripts/run_rust_live_submit_gate.sh}"
ACCOUNT_MONITOR_BIN_DEFAULT="${ACCOUNT_MONITOR_BIN_DEFAULT:-$ROOT/scripts/run_rust_show_account_info.sh}"

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
)

CLI_PROXY="$(proxy_from_args "$@" || true)"
if [[ -n "${PROXY_DEFAULT}" && -z "${CLI_PROXY}" ]]; then
  args+=(--proxy "$PROXY_DEFAULT")
fi

args+=(--watch-bin "$WATCH_BIN_DEFAULT")
args+=(--live-submit-bin "$LIVE_SUBMIT_BIN_DEFAULT")
args+=(--account-monitor-bin "$ACCOUNT_MONITOR_BIN_DEFAULT")

args+=("$@")

echo "== rust minmax follow =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
if [[ -n "${CLI_PROXY}" ]]; then
  echo "proxy=$CLI_PROXY"
elif [[ -n "${PROXY_DEFAULT}" ]]; then
  echo "proxy=$PROXY_DEFAULT"
else
  echo "proxy=disabled"
fi
echo "watch_bin=$WATCH_BIN_DEFAULT"
echo "live_submit_bin=$LIVE_SUBMIT_BIN_DEFAULT"
echo "account_monitor_bin=$ACCOUNT_MONITOR_BIN_DEFAULT"
echo

"$CARGO_BIN" "${args[@]}"
