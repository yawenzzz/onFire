#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FOLLOW_BIN="${FOLLOW_BIN:-$ROOT/scripts/run_rust_minmax_follow_live_submit.sh}"
FILL_LATENCY_LOGGER_BIN="${FILL_LATENCY_LOGGER_BIN:-$ROOT/scripts/run_rust_copytrade_fill_latency_logger.sh}"

USER_WALLET=""
ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      USER_WALLET="$2"
      ARGS+=("$1" "$2")
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
LOG_ROOT="${LOG_ROOT:-$ROOT/.omx/fill-latency/$LEADER_KEY}"
FOLLOW_STDOUT_LOG="$LOG_ROOT/follow.stdout.log"
FOLLOW_STDERR_LOG="$LOG_ROOT/follow.stderr.log"

mkdir -p "$LOG_ROOT"

echo "live_submit_latency_ready user=$USER_WALLET fills_log=$LOG_ROOT/fills.log fills_jsonl=$LOG_ROOT/fills.jsonl follow_stdout=$FOLLOW_STDOUT_LOG follow_stderr=$FOLLOW_STDERR_LOG"

bash "$FILL_LATENCY_LOGGER_BIN" --user "$USER_WALLET" --log-dir "$LOG_ROOT" &
LOGGER_PID=$!

bash "$FOLLOW_BIN" "${ARGS[@]}" >>"$FOLLOW_STDOUT_LOG" 2>>"$FOLLOW_STDERR_LOG" &
FOLLOW_PID=$!

cleanup() {
  if [[ -n "${LOGGER_PID:-}" ]]; then
    kill "$LOGGER_PID" 2>/dev/null || true
    wait "$LOGGER_PID" 2>/dev/null || true
  fi
  if [[ -n "${FOLLOW_PID:-}" ]]; then
    kill "$FOLLOW_PID" 2>/dev/null || true
    wait "$FOLLOW_PID" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

while true; do
  if ! kill -0 "$LOGGER_PID" 2>/dev/null; then
    wait "$LOGGER_PID"
    LOGGER_EXIT=$?
    wait "$FOLLOW_PID" 2>/dev/null || true
    exit "$LOGGER_EXIT"
  fi
  if ! kill -0 "$FOLLOW_PID" 2>/dev/null; then
    wait "$FOLLOW_PID"
    FOLLOW_EXIT=$?
    wait "$LOGGER_PID" 2>/dev/null || true
    exit "$FOLLOW_EXIT"
  fi
  sleep 1
done
