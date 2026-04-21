#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORCE_FOLLOW_ONCE_BIN="${FORCE_FOLLOW_ONCE_BIN:-$ROOT/scripts/run_rust_follow_last_action_force_live_once.sh}"
FOLLOW_FOREVER="${FOLLOW_FOREVER:-1}"
RESTART_ON_FAILURE="${RESTART_ON_FAILURE:-1}"
MAX_RESTARTS="${MAX_RESTARTS:-20}"
RESTART_DELAY_SECONDS="${RESTART_DELAY_SECONDS:-5}"
LOOP_DELAY_SECONDS="${LOOP_DELAY_SECONDS:-1}"

cd "$ROOT"
echo "== rust live submit continuous follow =="
echo "root=$ROOT"
echo "force_follow_once_bin=$FORCE_FOLLOW_ONCE_BIN"
echo "follow_forever=$FOLLOW_FOREVER"
echo "restart_on_failure=$RESTART_ON_FAILURE"
echo "max_restarts=$MAX_RESTARTS"
echo "restart_delay_seconds=$RESTART_DELAY_SECONDS"
echo "loop_delay_seconds=$LOOP_DELAY_SECONDS"
echo

restart_count=0
while true; do
  set +e
  REQUIRE_NEW_ACTIVITY=1 bash "$FORCE_FOLLOW_ONCE_BIN" "$@"
  run_exit=$?
  set -e

  if [[ "$run_exit" -eq 0 ]]; then
    restart_count=0
    if [[ "$FOLLOW_FOREVER" == "0" ]]; then
      exit 0
    fi
    sleep "$LOOP_DELAY_SECONDS"
    continue
  fi

  echo "force follow once failed (exit=$run_exit)" >&2
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
