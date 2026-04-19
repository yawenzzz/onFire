#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROXY_DEFAULT="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"
MIN_OPEN_USDC="${MIN_OPEN_USDC:-1}"
MAX_OPEN_USDC="${MAX_OPEN_USDC:-100}"
LOOP_INTERVAL_MS="${LOOP_INTERVAL_MS:-500}"
WATCH_LIMIT="${WATCH_LIMIT:-50}"

cd "$ROOT"

echo "== rust minmax follow live =="
echo "root=$ROOT"
echo "proxy=$PROXY_DEFAULT"
echo "min_open_usdc=$MIN_OPEN_USDC"
echo "max_open_usdc=$MAX_OPEN_USDC"
echo "loop_interval_ms=$LOOP_INTERVAL_MS"
echo "watch_limit=$WATCH_LIMIT"
echo

bash scripts/run_rust_minmax_follow.sh \
  --watch-limit "$WATCH_LIMIT" \
  --min-open-usdc "$MIN_OPEN_USDC" \
  --max-open-usdc "$MAX_OPEN_USDC" \
  --loop-interval-ms "$LOOP_INTERVAL_MS" \
  --forever \
  --activity-source-verified \
  --activity-under-budget \
  --activity-capability-detected \
  --positions-under-budget \
  --allow-live-submit \
  "$@"
