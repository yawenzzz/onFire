#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
USER_WALLET=""
REPORT_PATH=""
SOURCE_MODE="auto"
JSON_MODE=0

usage() {
  cat <<'EOF'
usage: bash scripts/run_rust_copytrade_latency_report.sh [--user <wallet>] [--report <path>] [--source auto|minmax|force-live] [--json]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      USER_WALLET="$2"
      shift 2
      ;;
    --report)
      REPORT_PATH="$2"
      shift 2
      ;;
    --source)
      SOURCE_MODE="$2"
      shift 2
      ;;
    --json)
      JSON_MODE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

resolve_report_path() {
  local wallet="$1"
  local source="$2"
  if [[ -n "$REPORT_PATH" ]]; then
    printf '%s\n' "$REPORT_PATH"
    return 0
  fi
  if [[ -z "$wallet" ]]; then
    echo "missing --user <wallet> when --report is not provided" >&2
    exit 2
  fi

  local minmax="$ROOT/.omx/minmax-follow/$wallet/latest.txt"
  local force_latest="$ROOT/.omx/force-live-follow/$wallet/latest-run.txt"
  if [[ "$source" == "minmax" ]]; then
    printf '%s\n' "$minmax"
    return 0
  fi
  if [[ "$source" == "force-live" ]]; then
    if [[ -f "$force_latest" ]]; then
      local run_dir
      run_dir="$(cat "$force_latest")"
      printf '%s/summary.txt\n' "$run_dir"
      return 0
    fi
    printf '%s\n' "$ROOT/.omx/force-live-follow/$wallet/summary.txt"
    return 0
  fi

  if [[ -f "$minmax" ]]; then
    printf '%s\n' "$minmax"
    return 0
  fi
  if [[ -f "$force_latest" ]]; then
    local run_dir
    run_dir="$(cat "$force_latest")"
    printf '%s/summary.txt\n' "$run_dir"
    return 0
  fi
  printf '%s\n' "$minmax"
}

has_latency_fields() {
  local file="$1"
  [[ -f "$file" ]] || return 1
  grep -q '^watch_elapsed_ms=' "$file"
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

kv() {
  local file="$1"
  local key="$2"
  grep -m1 "^${key}=" "$file" | sed "s/^${key}=//" || true
}

REPORT_PATH="$(resolve_report_path "$USER_WALLET" "$SOURCE_MODE")"
if [[ "$SOURCE_MODE" == "auto" && -n "$USER_WALLET" ]]; then
  FORCE_LATEST="$ROOT/.omx/force-live-follow/$USER_WALLET/latest-run.txt"
  if [[ -f "$REPORT_PATH" && -f "$FORCE_LATEST" ]] && ! has_latency_fields "$REPORT_PATH"; then
    FORCE_RUN_DIR="$(cat "$FORCE_LATEST")"
    if [[ -f "$FORCE_RUN_DIR/summary.txt" ]]; then
      REPORT_PATH="$FORCE_RUN_DIR/summary.txt"
    fi
  fi
fi
if [[ ! -f "$REPORT_PATH" ]]; then
  echo "report not found: $REPORT_PATH" >&2
  exit 1
fi

USER_WALLET="${USER_WALLET:-$(kv "$REPORT_PATH" user)}"
LATEST_TX="$(kv "$REPORT_PATH" latest_tx)"
LEADER_TS="$(kv "$REPORT_PATH" latest_activity_timestamp)"
if [[ -z "$LEADER_TS" ]]; then
  LEADER_TS="$(kv "$REPORT_PATH" latest_timestamp)"
fi
LEADER_PRICE="$(kv "$REPORT_PATH" latest_activity_price)"
if [[ -z "$LEADER_PRICE" ]]; then
  LEADER_PRICE="$(kv "$REPORT_PATH" activity_price)"
fi
WATCH_STARTED="$(kv "$REPORT_PATH" watch_started_at_unix_ms)"
WATCH_FINISHED="$(kv "$REPORT_PATH" watch_finished_at_unix_ms)"
WATCH_ELAPSED="$(kv "$REPORT_PATH" watch_elapsed_ms)"
LEADER_TO_WATCH="$(kv "$REPORT_PATH" leader_to_watch_finished_ms)"
GATE_STARTED="$(kv "$REPORT_PATH" gate_started_at_unix_ms)"
PAYLOAD_BUILD_STARTED="$(kv "$REPORT_PATH" payload_build_started_at_unix_ms)"
ORDER_BUILT="$(kv "$REPORT_PATH" order_built_at_unix_ms)"
ORDER_BUILD_ELAPSED="$(kv "$REPORT_PATH" order_build_elapsed_ms)"
PAYLOAD_READY="$(kv "$REPORT_PATH" payload_ready_at_unix_ms)"
PAYLOAD_PREP_ELAPSED="$(kv "$REPORT_PATH" payload_prep_elapsed_ms)"
LEADER_TO_PAYLOAD_READY="$(kv "$REPORT_PATH" leader_to_payload_ready_ms)"
FOLLOWER_PRICE="$(kv "$REPORT_PATH" follower_effective_price)"
PRICE_GAP="$(kv "$REPORT_PATH" price_gap)"
PRICE_GAP_BPS="$(kv "$REPORT_PATH" price_gap_bps)"
ADVERSE_GAP_BPS="$(kv "$REPORT_PATH" adverse_price_gap_bps)"
SUBMIT_STARTED="$(kv "$REPORT_PATH" submit_started_at_unix_ms)"
SUBMIT_FINISHED="$(kv "$REPORT_PATH" submit_finished_at_unix_ms)"
SUBMIT_ROUNDTRIP="$(kv "$REPORT_PATH" submit_roundtrip_elapsed_ms)"
LEADER_TO_SUBMIT_STARTED="$(kv "$REPORT_PATH" leader_to_submit_started_ms)"
LEADER_TO_SUBMIT_FINISHED="$(kv "$REPORT_PATH" leader_to_submit_finished_ms)"
STATUS="$(kv "$REPORT_PATH" status)"
if [[ -z "$STATUS" ]]; then
  STATUS="$(kv "$REPORT_PATH" submit_status)"
fi
if [[ -z "$STATUS" ]]; then
  STATUS="$(kv "$REPORT_PATH" live_submit_status)"
fi

capture_to_payload_ready=""
capture_to_submit_started=""
capture_to_submit_finished=""
gate_queue_delay=""
if [[ -n "$WATCH_FINISHED" && -n "$PAYLOAD_READY" ]]; then
  capture_to_payload_ready="$((PAYLOAD_READY - WATCH_FINISHED))"
fi
if [[ -n "$WATCH_FINISHED" && -n "$SUBMIT_STARTED" ]]; then
  capture_to_submit_started="$((SUBMIT_STARTED - WATCH_FINISHED))"
fi
if [[ -n "$WATCH_FINISHED" && -n "$SUBMIT_FINISHED" ]]; then
  capture_to_submit_finished="$((SUBMIT_FINISHED - WATCH_FINISHED))"
fi
if [[ -n "$WATCH_FINISHED" && -n "$GATE_STARTED" ]]; then
  gate_queue_delay="$((GATE_STARTED - WATCH_FINISHED))"
fi

if [[ "$JSON_MODE" == "1" ]]; then
  cat <<EOF
{
  "user": "$(json_escape "$USER_WALLET")",
  "report_path": "$(json_escape "$REPORT_PATH")",
  "status": "$(json_escape "$STATUS")",
  "latest_tx": "$(json_escape "$LATEST_TX")",
  "leader": {
    "timestamp": "$(json_escape "$LEADER_TS")",
    "price": "$(json_escape "$LEADER_PRICE")"
  },
  "watch": {
    "started_at_unix_ms": "$(json_escape "$WATCH_STARTED")",
    "finished_at_unix_ms": "$(json_escape "$WATCH_FINISHED")",
    "elapsed_ms": "$(json_escape "$WATCH_ELAPSED")",
    "leader_to_watch_finished_ms": "$(json_escape "$LEADER_TO_WATCH")"
  },
  "payload": {
    "build_started_at_unix_ms": "$(json_escape "$PAYLOAD_BUILD_STARTED")",
    "order_built_at_unix_ms": "$(json_escape "$ORDER_BUILT")",
    "order_build_elapsed_ms": "$(json_escape "$ORDER_BUILD_ELAPSED")",
    "ready_at_unix_ms": "$(json_escape "$PAYLOAD_READY")",
    "prep_elapsed_ms": "$(json_escape "$PAYLOAD_PREP_ELAPSED")",
    "capture_to_payload_ready_ms": "$(json_escape "$capture_to_payload_ready")",
    "leader_to_payload_ready_ms": "$(json_escape "$LEADER_TO_PAYLOAD_READY")",
    "gate_queue_delay_ms": "$(json_escape "$gate_queue_delay")"
  },
  "submit": {
    "started_at_unix_ms": "$(json_escape "$SUBMIT_STARTED")",
    "finished_at_unix_ms": "$(json_escape "$SUBMIT_FINISHED")",
    "roundtrip_elapsed_ms": "$(json_escape "$SUBMIT_ROUNDTRIP")",
    "capture_to_submit_started_ms": "$(json_escape "$capture_to_submit_started")",
    "capture_to_submit_finished_ms": "$(json_escape "$capture_to_submit_finished")",
    "leader_to_submit_started_ms": "$(json_escape "$LEADER_TO_SUBMIT_STARTED")",
    "leader_to_submit_finished_ms": "$(json_escape "$LEADER_TO_SUBMIT_FINISHED")"
  },
  "pricing": {
    "leader_price": "$(json_escape "$LEADER_PRICE")",
    "follower_effective_price": "$(json_escape "$FOLLOWER_PRICE")",
    "price_gap": "$(json_escape "$PRICE_GAP")",
    "price_gap_bps": "$(json_escape "$PRICE_GAP_BPS")",
    "adverse_price_gap_bps": "$(json_escape "$ADVERSE_GAP_BPS")"
  }
}
EOF
  exit 0
fi

cat <<EOF
== copytrade latency report ==
user=$USER_WALLET
report_path=$REPORT_PATH
status=$STATUS
latest_tx=$LATEST_TX

[leader]
leader_timestamp=$LEADER_TS
leader_price=$LEADER_PRICE

[watch]
watch_started_at_unix_ms=$WATCH_STARTED
watch_finished_at_unix_ms=$WATCH_FINISHED
watch_elapsed_ms=$WATCH_ELAPSED
leader_to_watch_finished_ms=$LEADER_TO_WATCH

[payload]
gate_started_at_unix_ms=$GATE_STARTED
payload_build_started_at_unix_ms=$PAYLOAD_BUILD_STARTED
order_built_at_unix_ms=$ORDER_BUILT
order_build_elapsed_ms=$ORDER_BUILD_ELAPSED
payload_ready_at_unix_ms=$PAYLOAD_READY
payload_prep_elapsed_ms=$PAYLOAD_PREP_ELAPSED
gate_queue_delay_ms=$gate_queue_delay
capture_to_payload_ready_ms=$capture_to_payload_ready
leader_to_payload_ready_ms=$LEADER_TO_PAYLOAD_READY

[submit]
submit_started_at_unix_ms=$SUBMIT_STARTED
submit_finished_at_unix_ms=$SUBMIT_FINISHED
submit_roundtrip_elapsed_ms=$SUBMIT_ROUNDTRIP
capture_to_submit_started_ms=$capture_to_submit_started
capture_to_submit_finished_ms=$capture_to_submit_finished
leader_to_submit_started_ms=$LEADER_TO_SUBMIT_STARTED
leader_to_submit_finished_ms=$LEADER_TO_SUBMIT_FINISHED

[pricing]
follower_effective_price=$FOLLOWER_PRICE
price_gap=$PRICE_GAP
price_gap_bps=$PRICE_GAP_BPS
adverse_price_gap_bps=$ADVERSE_GAP_BPS
EOF
