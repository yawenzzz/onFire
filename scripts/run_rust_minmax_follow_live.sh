#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"
MIN_OPEN_USDC="${MIN_OPEN_USDC:-1}"
MAX_OPEN_USDC="${MAX_OPEN_USDC:-100}"
MAX_TOTAL_EXPOSURE_USDC="${MAX_TOTAL_EXPOSURE_USDC:-100}"
MAX_ORDER_USDC="${MAX_ORDER_USDC:-10}"
ACCOUNT_SNAPSHOT_PATH="${ACCOUNT_SNAPSHOT_PATH:-runtime-verify-account/dashboard.json}"
ACCOUNT_SNAPSHOT_MAX_AGE_SECS="${ACCOUNT_SNAPSHOT_MAX_AGE_SECS:-300}"
LOOP_INTERVAL_MS="${LOOP_INTERVAL_MS:-500}"
WATCH_LIMIT="${WATCH_LIMIT:-50}"
FOLLOW_FOREVER="${FOLLOW_FOREVER:-0}"
AUTO_SUBMIT="${AUTO_SUBMIT:-0}"
RESTART_ON_FAILURE="${RESTART_ON_FAILURE:-1}"
MAX_RESTARTS="${MAX_RESTARTS:-20}"
RESTART_DELAY_SECONDS="${RESTART_DELAY_SECONDS:-5}"

cd "$ROOT"

echo "== rust minmax follow live =="
echo "root=$ROOT"
echo "proxy=$PROXY_DEFAULT"
echo "min_open_usdc=$MIN_OPEN_USDC"
echo "max_open_usdc=$MAX_OPEN_USDC"
echo "max_total_exposure_usdc=$MAX_TOTAL_EXPOSURE_USDC"
echo "max_order_usdc=$MAX_ORDER_USDC"
echo "account_snapshot_path=$ACCOUNT_SNAPSHOT_PATH"
echo "account_snapshot_max_age_secs=$ACCOUNT_SNAPSHOT_MAX_AGE_SECS"
echo "loop_interval_ms=$LOOP_INTERVAL_MS"
echo "watch_limit=$WATCH_LIMIT"
echo "follow_forever=$FOLLOW_FOREVER"
echo "auto_submit=$AUTO_SUBMIT"
echo "restart_on_failure=$RESTART_ON_FAILURE"
echo "max_restarts=$MAX_RESTARTS"
echo "restart_delay_seconds=$RESTART_DELAY_SECONDS"
echo

args=(
  --watch-limit "$WATCH_LIMIT"
  --min-open-usdc "$MIN_OPEN_USDC"
  --max-open-usdc "$MAX_OPEN_USDC"
  --max-total-exposure-usdc "$MAX_TOTAL_EXPOSURE_USDC"
  --max-order-usdc "$MAX_ORDER_USDC"
  --account-snapshot "$ACCOUNT_SNAPSHOT_PATH"
  --account-snapshot-max-age-secs "$ACCOUNT_SNAPSHOT_MAX_AGE_SECS"
  --loop-interval-ms "$LOOP_INTERVAL_MS"
)

if [[ "$FOLLOW_FOREVER" != "0" ]]; then
  args+=(--forever)
fi

if [[ "$AUTO_SUBMIT" != "0" ]]; then
  args+=(--allow-live-submit)
fi

restart_count=0
while true; do
  set +e
  bash scripts/run_rust_minmax_follow.sh "${args[@]}" "$@"
  run_exit=$?
  set -e

  if [[ "$run_exit" -eq 0 ]]; then
    exit 0
  fi

  echo "run_rust_minmax_follow.sh failed (exit=$run_exit)" >&2
  if [[ "$RESTART_ON_FAILURE" == "0" ]]; then
    exit "$run_exit"
  fi

  restart_count=$((restart_count + 1))
  if [[ "$MAX_RESTARTS" != "0" && "$restart_count" -gt "$MAX_RESTARTS" ]]; then
    echo "max restarts exceeded" >&2
    exit "$run_exit"
  fi
  sleep "$RESTART_DELAY_SECONDS"
done
