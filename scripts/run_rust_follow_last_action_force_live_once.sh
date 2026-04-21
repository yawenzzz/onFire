#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
WATCH_BIN_DEFAULT="${WATCH_BIN_DEFAULT:-$ROOT/scripts/run_rust_watch_copy_leader_activity.sh}"
LIVE_SUBMIT_BIN_DEFAULT="${LIVE_SUBMIT_BIN_DEFAULT:-$ROOT/scripts/run_rust_live_submit_gate.sh}"
CTF_ACTION_BIN_DEFAULT="${CTF_ACTION_BIN_DEFAULT:-$ROOT/scripts/run_rust_ctf_action.sh}"
WATCH_LIMIT="${WATCH_LIMIT:-50}"
WATCH_RETRY_COUNT="${WATCH_RETRY_COUNT:-3}"
WATCH_RETRY_DELAY_MS="${WATCH_RETRY_DELAY_MS:-1000}"
WATCH_ACTIVITY_TYPES="${WATCH_ACTIVITY_TYPES:-TRADE,MERGE,SPLIT}"
OPEN_USDC="${OPEN_USDC:-1}"
IGNORE_SEEN_TX="${IGNORE_SEEN_TX:-0}"
REQUIRE_NEW_ACTIVITY="${REQUIRE_NEW_ACTIVITY:-0}"

USER_WALLET=""
PROXY_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      USER_WALLET="$2"
      shift 2
      ;;
    --proxy)
      PROXY_OVERRIDE="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$USER_WALLET" ]]; then
  echo "missing --user <wallet>" >&2
  exit 2
fi

PROXY_DEFAULT="${PROXY_OVERRIDE:-${POLYMARKET_CURL_PROXY:-}}"

extract_latest_tx() {
  local latest_activity="$1"
  [[ -f "$latest_activity" ]] || return 0

  perl -MJSON::PP -e '
    my ($path) = @ARGV;
    local $/;
    open my $fh, "<", $path or exit 0;
    my $body = <$fh>;
    my $decoded = eval { JSON::PP->new->decode($body) };
    exit 0 if $@;
    my @items =
      ref($decoded) eq "ARRAY" ? @$decoded :
      ref($decoded) eq "HASH"  ? ($decoded) :
      ();
    my ($best_tx, $best_ts);
    for my $item (@items) {
      next unless ref($item) eq "HASH";
      my $tx = $item->{transactionHash};
      my $ts = $item->{timestamp};
      next unless defined $tx;
      $ts = 0 unless defined $ts;
      if (!defined($best_tx) || $ts > $best_ts) {
        $best_tx = $tx;
        $best_ts = $ts;
      }
    }
    print $best_tx if defined $best_tx;
  ' "$latest_activity" 2>/dev/null || true
}

extract_metric_value() {
  local metric_file="$1"
  local key="$2"
  [[ -f "$metric_file" ]] || return 0
  grep -m1 "^${key}=" "$metric_file" | sed -E "s/^${key}=//" || true
}

extract_event_field_by_tx() {
  local latest_activity="$1"
  local tx="$2"
  local field="$3"
  [[ -f "$latest_activity" ]] || return 0
  [[ -n "$tx" ]] || return 0

  perl -MJSON::PP -e '
    my ($path, $tx, $field) = @ARGV;
    local $/;
    open my $fh, "<", $path or exit 0;
    my $body = <$fh>;
    my $decoded = eval { JSON::PP->new->decode($body) };
    exit 0 if $@;
    my @items =
      ref($decoded) eq "ARRAY" ? @$decoded :
      ref($decoded) eq "HASH"  ? ($decoded) :
      ();
    for my $item (@items) {
      next unless ref($item) eq "HASH";
      my $candidate = $item->{transactionHash};
      next unless defined $candidate && $candidate eq $tx;
      my $value = $item->{$field};
      next if ref($value);
      next unless defined $value;
      print $value;
      last;
    }
  ' "$latest_activity" "$tx" "$field" 2>/dev/null || true
}

extract_event_json_by_tx() {
  local latest_activity="$1"
  local tx="$2"
  [[ -f "$latest_activity" ]] || return 0
  [[ -n "$tx" ]] || return 0

  perl -MJSON::PP -e '
    my ($path, $tx) = @ARGV;
    local $/;
    open my $fh, "<", $path or exit 0;
    my $body = <$fh>;
    my $decoded = eval { JSON::PP->new->decode($body) };
    exit 0 if $@;
    my @items =
      ref($decoded) eq "ARRAY" ? @$decoded :
      ref($decoded) eq "HASH"  ? ($decoded) :
      ();
    for my $item (@items) {
      next unless ref($item) eq "HASH";
      my $candidate = $item->{transactionHash};
      next unless defined $candidate && $candidate eq $tx;
      print JSON::PP->new->canonical->encode([$item]);
      last;
    }
  ' "$latest_activity" "$tx" 2>/dev/null || true
}

extract_latest_number_field() {
  local latest_activity="$1"
  local field="$2"
  [[ -f "$latest_activity" ]] || return 0
  local match=""
  match="$(grep -oE "\"${field}\"[[:space:]]*:[[:space:]]*[0-9.]+" "$latest_activity" | head -n1 || true)"
  [[ -n "$match" ]] || return 0
  printf '%s\n' "$match" | sed -E 's/.*:[[:space:]]*([0-9.]+)/\1/'
}

now_unix_ms() {
  if command -v perl >/dev/null 2>&1; then
    perl -MTime::HiRes=time -e 'printf("%.0f\n", time()*1000)'
  else
    printf '%s000\n' "$(date +%s)"
  fi
}

STATE_ROOT="$ROOT/.omx/force-live-follow/$USER_WALLET"
SELECTED_ENV="$STATE_ROOT/selected-leader.env"
LATEST_ACTIVITY="$ROOT/.omx/live-activity/$USER_WALLET/latest-activity.json"
RUNS_DIR="$STATE_ROOT/runs"
RUN_ID="$(date +%s%N)"
RUN_DIR="$RUNS_DIR/$RUN_ID"
LATEST_RUN_FILE="$STATE_ROOT/latest-run.txt"
WATCH_STDOUT="$RUN_DIR/watch.stdout.log"
WATCH_STDERR="$RUN_DIR/watch.stderr.log"
SUBMIT_STDOUT="$RUN_DIR/submit.stdout.log"
SUBMIT_STDERR="$RUN_DIR/submit.stderr.log"
SUMMARY="$RUN_DIR/summary.txt"
LAST_SUBMITTED_TX_FILE="$STATE_ROOT/last-submitted-tx.txt"
SELECTED_ACTIVITY="$RUN_DIR/latest-activity.selected.json"

mkdir -p "$STATE_ROOT"
mkdir -p "$RUN_DIR"
printf '%s\n' "$RUN_DIR" > "$LATEST_RUN_FILE"
cat > "$SELECTED_ENV" <<EOF_ENV
COPYTRADER_DISCOVERY_WALLET=$USER_WALLET
COPYTRADER_LEADER_WALLET=$USER_WALLET
COPYTRADER_SELECTED_FROM=force_live_follow_once
EOF_ENV

WATCH_ARGS=(
  --root
  ..
  --user
  "$USER_WALLET"
  --limit
  "$WATCH_LIMIT"
  --poll-count
  1
  --activity-type
  "$WATCH_ACTIVITY_TYPES"
  --retry-count
  "$WATCH_RETRY_COUNT"
  --retry-delay-ms
  "$WATCH_RETRY_DELAY_MS"
)

if [[ -n "$PROXY_DEFAULT" ]]; then
  WATCH_ARGS+=(--proxy "$PROXY_DEFAULT")
  export HTTPS_PROXY="${HTTPS_PROXY:-$PROXY_DEFAULT}"
  export HTTP_PROXY="${HTTP_PROXY:-$PROXY_DEFAULT}"
  export ALL_PROXY="${ALL_PROXY:-$PROXY_DEFAULT}"
fi

echo "== rust follow last action force live once =="
echo "root=$ROOT"
echo "cargo=$CARGO_BIN"
echo "user=$USER_WALLET"
echo "proxy=${PROXY_DEFAULT:-disabled}"
echo "submit_proxy=${HTTPS_PROXY:-disabled}"
echo "watch_limit=$WATCH_LIMIT"
echo "watch_retry_count=$WATCH_RETRY_COUNT"
echo "watch_retry_delay_ms=$WATCH_RETRY_DELAY_MS"
echo "watch_activity_types=$WATCH_ACTIVITY_TYPES"
echo "open_usdc=$OPEN_USDC"
echo "ignore_seen_tx=$IGNORE_SEEN_TX"
echo "require_new_activity=$REQUIRE_NEW_ACTIVITY"
echo "run_dir=$RUN_DIR"
echo

WATCH_STARTED_AT_UNIX_MS="$(now_unix_ms)"
set +e
"$WATCH_BIN_DEFAULT" "${WATCH_ARGS[@]}" >"$WATCH_STDOUT" 2>"$WATCH_STDERR"
WATCH_EXIT=$?
set -e
WATCH_FINISHED_AT_UNIX_MS="$(now_unix_ms)"
WATCH_ELAPSED_MS="$((WATCH_FINISHED_AT_UNIX_MS - WATCH_STARTED_AT_UNIX_MS))"

if [[ -f "$WATCH_STDOUT" ]]; then
  cat "$WATCH_STDOUT"
fi
if [[ -s "$WATCH_STDERR" ]]; then
  cat "$WATCH_STDERR" >&2
fi

if [[ "$WATCH_EXIT" -ne 0 ]]; then
  echo "watch_copy_leader_activity failed (exit=$WATCH_EXIT)" >&2
  if [[ ! -f "$LATEST_ACTIVITY" ]]; then
    cat > "$SUMMARY" <<EOF_SUMMARY
user=$USER_WALLET
watch_exit=$WATCH_EXIT
submit_exit=not_run
latest_activity=$LATEST_ACTIVITY
selected_leader_env=$SELECTED_ENV
run_dir=$RUN_DIR
status=watch_failed_no_cached_activity
EOF_SUMMARY
    exit "$WATCH_EXIT"
  fi
  echo "using cached latest activity: $LATEST_ACTIVITY" >&2
fi

WATCH_POLL_NEW_EVENTS="$(extract_metric_value "$WATCH_STDOUT" "poll_new_events")"
LATEST_TX="$(extract_metric_value "$WATCH_STDOUT" "latest_new_tx")"
if [[ -z "$LATEST_TX" ]]; then
  LATEST_TX="$(extract_latest_tx "$LATEST_ACTIVITY")"
fi
LATEST_ACTIVITY_TIMESTAMP="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "timestamp")"
if [[ -z "$LATEST_ACTIVITY_TIMESTAMP" ]]; then
  LATEST_ACTIVITY_TIMESTAMP="$(extract_latest_number_field "$LATEST_ACTIVITY" "timestamp")"
fi
LATEST_ACTIVITY_PRICE="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "price")"
if [[ -z "$LATEST_ACTIVITY_PRICE" ]]; then
  LATEST_ACTIVITY_PRICE="$(extract_latest_number_field "$LATEST_ACTIVITY" "price")"
fi
LATEST_ACTIVITY_TYPE="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "type")"
if [[ -z "$LATEST_ACTIVITY_TYPE" ]]; then
  LATEST_ACTIVITY_TYPE="TRADE"
fi
SELECTED_ACTIVITY_JSON="$(extract_event_json_by_tx "$LATEST_ACTIVITY" "$LATEST_TX")"
if [[ -n "$SELECTED_ACTIVITY_JSON" ]]; then
  printf '%s\n' "$SELECTED_ACTIVITY_JSON" > "$SELECTED_ACTIVITY"
else
  SELECTED_ACTIVITY="$LATEST_ACTIVITY"
fi
LEADER_TO_WATCH_FINISHED_MS=""
if [[ -n "$LATEST_ACTIVITY_TIMESTAMP" ]]; then
  LEADER_TO_WATCH_FINISHED_MS="$((WATCH_FINISHED_AT_UNIX_MS - (LATEST_ACTIVITY_TIMESTAMP * 1000)))"
fi

if [[ "$REQUIRE_NEW_ACTIVITY" == "1" && "${WATCH_POLL_NEW_EVENTS:-0}" == "0" ]]; then
  echo "skipping because watch reported no new activity"
  cat > "$SUMMARY" <<EOF_SUMMARY
user=$USER_WALLET
watch_exit=$WATCH_EXIT
submit_exit=not_run
latest_tx=$LATEST_TX
latest_activity_timestamp=$LATEST_ACTIVITY_TIMESTAMP
latest_activity_price=$LATEST_ACTIVITY_PRICE
latest_activity_type=$LATEST_ACTIVITY_TYPE
watch_started_at_unix_ms=$WATCH_STARTED_AT_UNIX_MS
watch_finished_at_unix_ms=$WATCH_FINISHED_AT_UNIX_MS
watch_elapsed_ms=$WATCH_ELAPSED_MS
leader_to_watch_finished_ms=$LEADER_TO_WATCH_FINISHED_MS
latest_activity=$LATEST_ACTIVITY
selected_latest_activity=$SELECTED_ACTIVITY
selected_leader_env=$SELECTED_ENV
watch_stdout_log=$WATCH_STDOUT
watch_stderr_log=$WATCH_STDERR
submit_stdout_log=$SUBMIT_STDOUT
submit_stderr_log=$SUBMIT_STDERR
run_dir=$RUN_DIR
status=no_new_activity_skipped
EOF_SUMMARY
  exit 0
fi

if [[ "$IGNORE_SEEN_TX" != "1" && -f "$LAST_SUBMITTED_TX_FILE" ]]; then
  LAST_SUBMITTED_TX="$(cat "$LAST_SUBMITTED_TX_FILE")"
  if [[ -n "$LATEST_TX" && "$LATEST_TX" == "$LAST_SUBMITTED_TX" ]]; then
    echo "skipping duplicate latest tx: $LATEST_TX"
    cat > "$SUMMARY" <<EOF_SUMMARY
user=$USER_WALLET
watch_exit=$WATCH_EXIT
submit_exit=not_run
latest_tx=$LATEST_TX
latest_activity_timestamp=$LATEST_ACTIVITY_TIMESTAMP
latest_activity_price=$LATEST_ACTIVITY_PRICE
latest_activity_type=$LATEST_ACTIVITY_TYPE
watch_started_at_unix_ms=$WATCH_STARTED_AT_UNIX_MS
watch_finished_at_unix_ms=$WATCH_FINISHED_AT_UNIX_MS
watch_elapsed_ms=$WATCH_ELAPSED_MS
leader_to_watch_finished_ms=$LEADER_TO_WATCH_FINISHED_MS
latest_activity=$LATEST_ACTIVITY
selected_latest_activity=$SELECTED_ACTIVITY
selected_leader_env=$SELECTED_ENV
watch_stdout_log=$WATCH_STDOUT
watch_stderr_log=$WATCH_STDERR
submit_stdout_log=$SUBMIT_STDOUT
submit_stderr_log=$SUBMIT_STDERR
run_dir=$RUN_DIR
status=duplicate_tx_skipped
EOF_SUMMARY
    exit 0
  fi
fi

ACTION_BIN="$LIVE_SUBMIT_BIN_DEFAULT"
ACTION_ARGS=(
  --root ..
  --selected-leader-env "$SELECTED_ENV"
  --latest-activity "$SELECTED_ACTIVITY"
  --override-usdc-size "$OPEN_USDC"
)
if [[ "$LATEST_ACTIVITY_TYPE" == "MERGE" || "$LATEST_ACTIVITY_TYPE" == "SPLIT" ]]; then
  ACTION_BIN="$CTF_ACTION_BIN_DEFAULT"
  ACTION_ARGS+=(--allow-live-submit)
else
  ACTION_ARGS+=(--allow-live-submit --force-live-submit)
fi

set +e
"$ACTION_BIN" "${ACTION_ARGS[@]}" >"$SUBMIT_STDOUT" 2>"$SUBMIT_STDERR"
SUBMIT_EXIT=$?
set -e

if [[ -f "$SUBMIT_STDOUT" ]]; then
  cat "$SUBMIT_STDOUT"
fi
if [[ -s "$SUBMIT_STDERR" ]]; then
  cat "$SUBMIT_STDERR" >&2
fi

submit_marks_seen() {
  if [[ ! -f "$SUBMIT_STDOUT" ]]; then
    return 1
  fi
  if grep -q '^ctf_action_status=submitted$' "$SUBMIT_STDOUT"; then
    return 0
  fi
  if grep -q '^live_submit_status=submitted$' "$SUBMIT_STDOUT"; then
    if grep -q '^submit_success=false$' "$SUBMIT_STDOUT"; then
      return 1
    fi
    return 0
  fi
  return 1
}

if [[ "$SUBMIT_EXIT" -eq 0 && -n "$LATEST_TX" ]] && submit_marks_seen; then
  printf '%s\n' "$LATEST_TX" > "$LAST_SUBMITTED_TX_FILE"
fi

metric_from_submit() {
  local key="$1"
  if [[ -f "$SUBMIT_STDOUT" ]]; then
    grep -m1 "^${key}=" "$SUBMIT_STDOUT" || true
  fi
}

cat > "$SUMMARY" <<EOF_SUMMARY
user=$USER_WALLET
watch_exit=$WATCH_EXIT
submit_exit=$SUBMIT_EXIT
latest_tx=$LATEST_TX
latest_activity_timestamp=$LATEST_ACTIVITY_TIMESTAMP
latest_activity_price=$LATEST_ACTIVITY_PRICE
latest_activity_type=$LATEST_ACTIVITY_TYPE
watch_started_at_unix_ms=$WATCH_STARTED_AT_UNIX_MS
watch_finished_at_unix_ms=$WATCH_FINISHED_AT_UNIX_MS
watch_elapsed_ms=$WATCH_ELAPSED_MS
leader_to_watch_finished_ms=$LEADER_TO_WATCH_FINISHED_MS
latest_activity=$LATEST_ACTIVITY
selected_latest_activity=$SELECTED_ACTIVITY
selected_leader_env=$SELECTED_ENV
watch_stdout_log=$WATCH_STDOUT
watch_stderr_log=$WATCH_STDERR
submit_stdout_log=$SUBMIT_STDOUT
submit_stderr_log=$SUBMIT_STDERR
$(metric_from_submit gate_started_at_unix_ms)
$(metric_from_submit payload_build_started_at_unix_ms)
$(metric_from_submit order_built_at_unix_ms)
$(metric_from_submit order_build_elapsed_ms)
$(metric_from_submit payload_ready_at_unix_ms)
$(metric_from_submit payload_prep_elapsed_ms)
$(metric_from_submit leader_to_payload_ready_ms)
$(metric_from_submit leader_price)
$(metric_from_submit follower_effective_price)
$(metric_from_submit price_gap)
$(metric_from_submit price_gap_bps)
$(metric_from_submit adverse_price_gap_bps)
$(metric_from_submit ctf_action_type)
$(metric_from_submit ctf_action_status)
$(metric_from_submit ctf_action_tx_hash)
$(metric_from_submit ctf_action_block_number)
$(metric_from_submit action_usdc_size)
$(metric_from_submit submit_started_at_unix_ms)
$(metric_from_submit submit_finished_at_unix_ms)
$(metric_from_submit submit_roundtrip_elapsed_ms)
$(metric_from_submit leader_to_submit_started_ms)
$(metric_from_submit leader_to_submit_finished_ms)
run_dir=$RUN_DIR
status=$(if [[ "$SUBMIT_EXIT" -eq 0 ]]; then echo submit_completed; else echo submit_failed; fi)
EOF_SUMMARY

exit "$SUBMIT_EXIT"
