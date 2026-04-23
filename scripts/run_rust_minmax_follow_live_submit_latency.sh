#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LATENCY_LOGGER_BIN="${LATENCY_LOGGER_BIN:-$ROOT/scripts/run_rust_copytrade_fill_latency_logger.sh}"

USER_WALLET=""
ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      USER_WALLET="$2"
      shift 2
      ;;
    *)
      ARGS+=("$1")
      shift
      ;;
  esac
done

if [[ -z "$USER_WALLET" ]]; then
  echo "missing --user <leader-wallet>" >&2
  exit 2
fi

LEADER_KEY="$(printf '%s' "$USER_WALLET" | tr -c '[:alnum:]_-' '_')"
LOG_ROOT="${LOG_ROOT:-$ROOT/logs/copytrade-fill-latency/$LEADER_KEY}"

mkdir -p "$LOG_ROOT"

echo "[info]: fill latency logger only"
echo "[info]: log_file=$LOG_ROOT/fills.log"
echo "[info]: run main follow separately: bash scripts/run_rust_minmax_follow_live_submit.sh --user $USER_WALLET"

cmd=(bash "$LATENCY_LOGGER_BIN" --user "$USER_WALLET" --log-dir "$LOG_ROOT")
if [[ ${#ARGS[@]} -gt 0 ]]; then
  cmd+=("${ARGS[@]}")
fi

exec "${cmd[@]}"
