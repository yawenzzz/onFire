#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"
WATCH_BIN_DEFAULT="${WATCH_BIN_DEFAULT:-$ROOT/scripts/run_rust_watch_copy_leader_activity.sh}"
LIVE_SUBMIT_BIN_DEFAULT="${LIVE_SUBMIT_BIN_DEFAULT:-$ROOT/scripts/run_rust_live_submit_gate.sh}"
CTF_ACTION_BIN_DEFAULT="${CTF_ACTION_BIN_DEFAULT:-$ROOT/scripts/run_rust_ctf_action.sh}"
POSITIONS_GATE_BIN_DEFAULT="${POSITIONS_GATE_BIN_DEFAULT:-$ROOT/scripts/run_rust_public_positions_gate.sh}"
ACCOUNT_SNAPSHOT_BIN_DEFAULT="${ACCOUNT_SNAPSHOT_BIN_DEFAULT:-$ROOT/scripts/run_rust_show_account_info.sh}"
ACCOUNT_SNAPSHOT_PATH="${ACCOUNT_SNAPSHOT_PATH:-runtime-verify-account/dashboard.json}"
WATCH_LIMIT="${WATCH_LIMIT:-50}"
WATCH_RETRY_COUNT="${WATCH_RETRY_COUNT:-3}"
WATCH_RETRY_DELAY_MS="${WATCH_RETRY_DELAY_MS:-1000}"
WATCH_ACTIVITY_TYPES="${WATCH_ACTIVITY_TYPES:-TRADE,MERGE,SPLIT}"
OPEN_USDC="${OPEN_USDC:-1}"
IGNORE_SEEN_TX="${IGNORE_SEEN_TX:-0}"
REQUIRE_NEW_ACTIVITY="${REQUIRE_NEW_ACTIVITY:-0}"
FOLLOW_SHARE_DIVISOR="${FOLLOW_SHARE_DIVISOR:-10}"
MIN_OPEN_SHARES="${MIN_OPEN_SHARES:-5}"
MIN_COMPATIBLE_SHARES="${MIN_COMPATIBLE_SHARES:-0.01}"
FOLLOW_ORDER_TYPE="${FOLLOW_ORDER_TYPE:-GTC}"
POSITIONS_RETRY_COUNT="${POSITIONS_RETRY_COUNT:-4}"
POSITIONS_RETRY_DELAY_MS="${POSITIONS_RETRY_DELAY_MS:-750}"
FORCE_FOLLOW_SKIP_WATCH="${FORCE_FOLLOW_SKIP_WATCH:-0}"
FORCE_FOLLOW_SELECTED_TX="${FORCE_FOLLOW_SELECTED_TX:-}"
FORCE_FOLLOW_OVERRIDE_NEW_OPEN="${FORCE_FOLLOW_OVERRIDE_NEW_OPEN:-0}"

USER_WALLET=""
PROXY_OVERRIDE=""
FOLLOW_SHARE_DIVISOR_OVERRIDE=""

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
    --follow-share-divisor)
      FOLLOW_SHARE_DIVISOR_OVERRIDE="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -n "$FOLLOW_SHARE_DIVISOR_OVERRIDE" ]]; then
  FOLLOW_SHARE_DIVISOR="$FOLLOW_SHARE_DIVISOR_OVERRIDE"
fi

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

divide_decimal() {
  local value="${1:-0}"
  local divisor="${2:-1}"
  perl -e '
    my ($value, $divisor) = @ARGV;
    $value = 0 unless defined $value && $value ne "";
    $divisor = 1 unless defined $divisor && $divisor ne "";
    die "divisor must not be zero\n" if $divisor == 0;
    printf("%.6f\n", ($value + 0) / ($divisor + 0));
  ' "$value" "$divisor"
}

scale_usdc_for_target_shares() {
  local original_usdc="${1:-0}"
  local original_shares="${2:-0}"
  local target_shares="${3:-0}"
  perl -e '
    my ($original_usdc, $original_shares, $target_shares) = @ARGV;
    exit 0 if ($original_shares + 0) <= 0;
    printf("%.6f\n", (($original_usdc + 0) * ($target_shares + 0)) / ($original_shares + 0));
  ' "$original_usdc" "$original_shares" "$target_shares"
}

decimal_lt() {
  local left="${1:-0}"
  local right="${2:-0}"
  perl -e '
    my ($left, $right) = @ARGV;
    exit(($left + 0) < ($right + 0) ? 0 : 1);
  ' "$left" "$right"
}

decimal_gt() {
  local left="${1:-0}"
  local right="${2:-0}"
  perl -e '
    my ($left, $right) = @ARGV;
    exit(($left + 0) > ($right + 0) ? 0 : 1);
  ' "$left" "$right"
}

metric_from_positions_gate() {
  local key="$1"
  if [[ -f "$POSITIONS_STDOUT" ]]; then
    grep -m1 "^${key}=" "$POSITIONS_STDOUT" || true
  fi
}

run_with_shell_fallback() {
  local program="$1"
  shift
  if [[ -x "$program" ]]; then
    "$program" "$@"
  else
    bash "$program" "$@"
  fi
}

resolve_under_root() {
  local path="$1"
  if [[ "$path" = /* ]]; then
    printf '%s\n' "$path"
  else
    printf '%s/%s\n' "$ROOT" "$path"
  fi
}

extract_snapshot_net_size_by_asset() {
  local snapshot_path="$1"
  local asset_id="$2"
  [[ -f "$snapshot_path" ]] || return 0
  [[ -n "$asset_id" ]] || return 0

  perl -MJSON::PP -e '
    my ($path, $asset_id) = @ARGV;
    local $/;
    open my $fh, "<", $path or exit 0;
    my $body = <$fh>;
    my $decoded = eval { JSON::PP->new->decode($body) };
    exit 0 if $@;
    my $snapshot =
      ref($decoded) eq "HASH" && ref($decoded->{account_snapshot}) eq "HASH"
        ? $decoded->{account_snapshot}
        : $decoded;
    my $positions = ref($snapshot) eq "HASH" ? $snapshot->{positions} : undef;
    exit 0 unless ref($positions) eq "ARRAY";
    my $sum = 0;
    for my $position (@$positions) {
      next unless ref($position) eq "HASH";
      my $candidate = $position->{asset_id};
      $candidate = $position->{asset} unless defined($candidate);
      next unless defined($candidate) && "$candidate" eq $asset_id;
      my $net_size = $position->{net_size};
      $net_size = $position->{size} unless defined($net_size);
      next unless defined $net_size;
      $sum += ($net_size + 0);
    }
    printf("%.6f\n", $sum);
  ' "$snapshot_path" "$asset_id" 2>/dev/null || true
}

extract_unseen_activity_plans() {
  local latest_activity="$1"
  local seen_before_path="$2"
  [[ -f "$latest_activity" ]] || return 0

  perl -MJSON::PP -e '
    my ($path, $seen_path) = @ARGV;
    my %seen_before;
    if (defined $seen_path && -f $seen_path) {
      open my $seen_fh, "<", $seen_path or exit 0;
      while (my $line = <$seen_fh>) {
        chomp $line;
        next unless defined $line && $line ne "";
        $seen_before{$line} = 1;
      }
      close $seen_fh;
    }
    local $/;
    open my $fh, "<", $path or exit 0;
    my $body = <$fh>;
    my $decoded = eval { JSON::PP->new->decode($body) };
    exit 0 if $@;
    my @items =
      ref($decoded) eq "ARRAY" ? @$decoded :
      ref($decoded) eq "HASH"  ? ($decoded) :
      ();
    my @records;
    my %event_has_prior_seen;
    for my $item (@items) {
      next unless ref($item) eq "HASH";
      my $tx = $item->{transactionHash};
      next unless defined $tx && $tx ne "";
      my $timestamp = defined($item->{timestamp}) ? ($item->{timestamp} + 0) : 0;
      my $type = defined($item->{type}) ? $item->{type} : "";
      my $side = defined($item->{side}) ? $item->{side} : "";
      my $event_key = $item->{conditionId};
      $event_key = $item->{eventSlug} unless defined($event_key) && $event_key ne "";
      $event_key = $item->{event_slug} unless defined($event_key) && $event_key ne "";
      $event_key = $item->{slug} unless defined($event_key) && $event_key ne "";
      $event_key = $item->{asset} unless defined($event_key) && $event_key ne "";
      push @records, {
        tx => $tx,
        timestamp => $timestamp,
        type => $type,
        side => $side,
        event_key => (defined($event_key) ? $event_key : ""),
        seen_before => ($seen_before{$tx} ? 1 : 0),
      };
      if ($seen_before{$tx} && defined($event_key) && $event_key ne "") {
        $event_has_prior_seen{$event_key} = 1;
      }
    }
    @records = sort { $a->{timestamp} <=> $b->{timestamp} } @records;
    for my $record (@records) {
      next if $record->{seen_before};
      my $override = (
        uc($record->{type} // "") eq "TRADE"
        && uc($record->{side} // "") eq "BUY"
        && $record->{event_key} ne ""
        && !$event_has_prior_seen{$record->{event_key}}
      ) ? 1 : 0;
      print join("\t",
        $record->{tx},
        $record->{timestamp},
        $record->{type},
        $record->{side},
        $record->{event_key},
        $override,
      ), "\n";
    }
  ' "$latest_activity" "$seen_before_path" 2>/dev/null || true
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
POSITIONS_STDOUT="$RUN_DIR/positions.stdout.log"
POSITIONS_STDERR="$RUN_DIR/positions.stderr.log"
ACCOUNT_SNAPSHOT_STDOUT="$RUN_DIR/account-snapshot.stdout.log"
ACCOUNT_SNAPSHOT_STDERR="$RUN_DIR/account-snapshot.stderr.log"
SUBMIT_STDOUT="$RUN_DIR/submit.stdout.log"
SUBMIT_STDERR="$RUN_DIR/submit.stderr.log"
SUMMARY="$RUN_DIR/summary.txt"
LAST_SUBMITTED_TX_FILE="$STATE_ROOT/last-submitted-tx.txt"
SELECTED_ACTIVITY="$RUN_DIR/latest-activity.selected.json"
WATCH_SEEN_TX_FILE="$ROOT/.omx/live-activity/$USER_WALLET/seen-tx.txt"
ACCOUNT_SNAPSHOT_FILE="$(resolve_under_root "$ACCOUNT_SNAPSHOT_PATH")"

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
echo "follow_share_divisor=$FOLLOW_SHARE_DIVISOR"
echo "min_open_shares=$MIN_OPEN_SHARES"
echo "min_compatible_shares=$MIN_COMPATIBLE_SHARES"
echo "follow_order_type=$FOLLOW_ORDER_TYPE"
echo "positions_gate_bin=$POSITIONS_GATE_BIN_DEFAULT"
echo "positions_retry_count=$POSITIONS_RETRY_COUNT"
echo "positions_retry_delay_ms=$POSITIONS_RETRY_DELAY_MS"
echo "account_snapshot_bin=$ACCOUNT_SNAPSHOT_BIN_DEFAULT"
echo "account_snapshot_path=$ACCOUNT_SNAPSHOT_FILE"
echo "run_dir=$RUN_DIR"
echo

SEEN_BEFORE_PATH="$RUN_DIR/seen.before.txt"
if [[ -f "$WATCH_SEEN_TX_FILE" ]]; then
  cp "$WATCH_SEEN_TX_FILE" "$SEEN_BEFORE_PATH"
else
  : > "$SEEN_BEFORE_PATH"
fi

WATCH_STARTED_AT_UNIX_MS="$(now_unix_ms)"
WATCH_EXIT=0
WATCH_POLL_NEW_EVENTS=0
if [[ "$FORCE_FOLLOW_SKIP_WATCH" == "1" ]]; then
  WATCH_FINISHED_AT_UNIX_MS="${FORCE_FOLLOW_WATCH_FINISHED_AT_UNIX_MS:-$WATCH_STARTED_AT_UNIX_MS}"
  WATCH_ELAPSED_MS="$((WATCH_FINISHED_AT_UNIX_MS - WATCH_STARTED_AT_UNIX_MS))"
  WATCH_POLL_NEW_EVENTS="${FORCE_FOLLOW_WATCH_POLL_NEW_EVENTS:-1}"
else
  set +e
  run_with_shell_fallback "$WATCH_BIN_DEFAULT" "${WATCH_ARGS[@]}" >"$WATCH_STDOUT" 2>"$WATCH_STDERR"
  WATCH_EXIT=$?
  set -e
  WATCH_FINISHED_AT_UNIX_MS="$(now_unix_ms)"
  WATCH_ELAPSED_MS="$((WATCH_FINISHED_AT_UNIX_MS - WATCH_STARTED_AT_UNIX_MS))"
fi

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

if [[ "$FORCE_FOLLOW_SKIP_WATCH" != "1" ]]; then
  WATCH_POLL_NEW_EVENTS="$(extract_metric_value "$WATCH_STDOUT" "poll_new_events")"
fi

if [[ "$FORCE_FOLLOW_SKIP_WATCH" != "1" && "${WATCH_POLL_NEW_EVENTS:-0}" -gt 1 ]]; then
  UNSEEN_ACTIVITY_PLANS=()
  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    UNSEEN_ACTIVITY_PLANS+=("$line")
  done <<EOF_UNSEEN
$(extract_unseen_activity_plans "$LATEST_ACTIVITY" "$SEEN_BEFORE_PATH")
EOF_UNSEEN
  if [[ ${#UNSEEN_ACTIVITY_PLANS[@]} -gt 1 ]]; then
    OVERALL_EXIT=0
    CHILD_ARGS=(--user "$USER_WALLET" --follow-share-divisor "$FOLLOW_SHARE_DIVISOR")
    if [[ -n "$PROXY_OVERRIDE" ]]; then
      CHILD_ARGS+=(--proxy "$PROXY_OVERRIDE")
    fi
    for plan in "${UNSEEN_ACTIVITY_PLANS[@]}"; do
      IFS=$'\t' read -r PLAN_TX PLAN_TS PLAN_TYPE PLAN_SIDE PLAN_EVENT_KEY PLAN_OVERRIDE <<<"$plan"
      set +e
      FORCE_FOLLOW_SKIP_WATCH=1 \
      FORCE_FOLLOW_SELECTED_TX="$PLAN_TX" \
      FORCE_FOLLOW_OVERRIDE_NEW_OPEN="$PLAN_OVERRIDE" \
      FORCE_FOLLOW_WATCH_FINISHED_AT_UNIX_MS="$WATCH_FINISHED_AT_UNIX_MS" \
      FORCE_FOLLOW_WATCH_POLL_NEW_EVENTS="$WATCH_POLL_NEW_EVENTS" \
      bash "$0" "${CHILD_ARGS[@]}"
      CHILD_EXIT=$?
      set -e
      if [[ "$CHILD_EXIT" -ne 0 ]]; then
        OVERALL_EXIT="$CHILD_EXIT"
        break
      fi
    done
    exit "$OVERALL_EXIT"
  fi
fi

if [[ -n "$FORCE_FOLLOW_SELECTED_TX" ]]; then
  LATEST_TX="$FORCE_FOLLOW_SELECTED_TX"
else
  LATEST_TX="$(extract_metric_value "$WATCH_STDOUT" "latest_new_tx")"
  if [[ -z "$LATEST_TX" ]]; then
    LATEST_TX="$(extract_latest_tx "$LATEST_ACTIVITY")"
  fi
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
LATEST_ACTIVITY_SIDE="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "side")"
LATEST_ACTIVITY_ASSET="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "asset")"
LATEST_ACTIVITY_SIZE="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "size")"
LATEST_ACTIVITY_USDC_SIZE="$(extract_event_field_by_tx "$LATEST_ACTIVITY" "$LATEST_TX" "usdcSize")"
SELECTED_ACTIVITY_JSON="$(extract_event_json_by_tx "$LATEST_ACTIVITY" "$LATEST_TX")"
if [[ -n "$SELECTED_ACTIVITY_JSON" ]]; then
  printf '%s\n' "$SELECTED_ACTIVITY_JSON" > "$SELECTED_ACTIVITY"
else
  SELECTED_ACTIVITY="$LATEST_ACTIVITY"
fi
if [[ -z "$LATEST_ACTIVITY_SIZE" ]]; then
  LATEST_ACTIVITY_SIZE="$(extract_latest_number_field "$SELECTED_ACTIVITY" "size")"
fi
if [[ -z "$LATEST_ACTIVITY_USDC_SIZE" ]]; then
  LATEST_ACTIVITY_USDC_SIZE="$(extract_latest_number_field "$SELECTED_ACTIVITY" "usdcSize")"
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

FOLLOW_SHARES=""
FOLLOW_USDC=""
POSITIONS_GATE_EXIT="not_run"
LATEST_ACTIVITY_SIDE_UPPER="$(printf '%s' "${LATEST_ACTIVITY_SIDE:-}" | tr '[:lower:]' '[:upper:]')"
LEADER_EVENT_SHOULD_FOLLOW=""
LEADER_EVENT_OPEN_GATE_STATUS=""
LEADER_EVENT_OPEN_GATE_REASON=""
FOLLOWER_SNAPSHOT_EXIT="not_run"
FOLLOWER_POSITION_CHECK_STATUS="not_run"
FOLLOWER_CURRENT_ASSET_NET_SIZE=""
FOLLOWER_CURRENT_ASSET_HELD="false"
FOLLOW_TRIGGER_REASON=""
FOLLOW_IS_FIRST_OPEN="false"
FOLLOW_MIN_OPEN_FLOOR_APPLIED="false"
FOLLOW_MIN_COMPATIBLE_FLOOR_APPLIED="false"

if [[ "$LATEST_ACTIVITY_TYPE" == "TRADE" ]]; then
  set +e
  run_with_shell_fallback "$POSITIONS_GATE_BIN_DEFAULT" \
    --user "$USER_WALLET" \
    --latest-activity "$SELECTED_ACTIVITY" \
    --positions-limit 500 \
    --positions-retry-count "$POSITIONS_RETRY_COUNT" \
    --positions-retry-delay-ms "$POSITIONS_RETRY_DELAY_MS" \
    >"$POSITIONS_STDOUT" 2>"$POSITIONS_STDERR"
  POSITIONS_GATE_EXIT=$?
  set -e

  if [[ -f "$POSITIONS_STDOUT" ]]; then
    cat "$POSITIONS_STDOUT"
  fi
  if [[ -s "$POSITIONS_STDERR" ]]; then
    cat "$POSITIONS_STDERR" >&2
  fi

  if [[ "$POSITIONS_GATE_EXIT" -ne 0 ]]; then
    echo "current positions gate failed (exit=$POSITIONS_GATE_EXIT); continuing with follower-held-position check" >&2
    LEADER_EVENT_SHOULD_FOLLOW="false"
    LEADER_EVENT_OPEN_GATE_STATUS="positions_gate_failed"
    LEADER_EVENT_OPEN_GATE_REASON="current_positions_gate_failed"
  else
    LEADER_EVENT_SHOULD_FOLLOW="$(extract_metric_value "$POSITIONS_STDOUT" "leader_event_should_follow")"
    LEADER_EVENT_OPEN_GATE_STATUS="$(extract_metric_value "$POSITIONS_STDOUT" "leader_event_open_gate_status")"
    LEADER_EVENT_OPEN_GATE_REASON="$(extract_metric_value "$POSITIONS_STDOUT" "leader_event_open_gate_reason")"
  fi
  set +e
  run_with_shell_fallback "$ACCOUNT_SNAPSHOT_BIN_DEFAULT" --output "$ACCOUNT_SNAPSHOT_PATH" >"$ACCOUNT_SNAPSHOT_STDOUT" 2>"$ACCOUNT_SNAPSHOT_STDERR"
  FOLLOWER_SNAPSHOT_EXIT=$?
  set -e
  if [[ "$FOLLOWER_SNAPSHOT_EXIT" -eq 0 ]]; then
    FOLLOWER_POSITION_CHECK_STATUS="ok"
    FOLLOWER_CURRENT_ASSET_NET_SIZE="$(extract_snapshot_net_size_by_asset "$ACCOUNT_SNAPSHOT_FILE" "$LATEST_ACTIVITY_ASSET")"
  else
    FOLLOWER_POSITION_CHECK_STATUS="refresh_failed"
  fi
  if [[ -n "$FOLLOWER_CURRENT_ASSET_NET_SIZE" ]] && decimal_gt "$FOLLOWER_CURRENT_ASSET_NET_SIZE" "0"; then
    FOLLOWER_CURRENT_ASSET_HELD="true"
  fi
  if [[ -n "$LATEST_ACTIVITY_SIZE" ]]; then
    FOLLOW_SHARES="$(divide_decimal "${LATEST_ACTIVITY_SIZE:-0}" "$FOLLOW_SHARE_DIVISOR")"
    FOLLOW_USDC="$(divide_decimal "${LATEST_ACTIVITY_USDC_SIZE:-0}" "$FOLLOW_SHARE_DIVISOR")"
  fi
fi

if [[ "$FORCE_FOLLOW_OVERRIDE_NEW_OPEN" == "1" ]]; then
  LEADER_EVENT_SHOULD_FOLLOW="true"
  LEADER_EVENT_OPEN_GATE_STATUS="batch_new_event_open"
  LEADER_EVENT_OPEN_GATE_REASON="unseen_trade_batch_without_prior_event_history"
fi

if [[ "$LATEST_ACTIVITY_TYPE" == "TRADE" ]]; then
  if [[ "$LEADER_EVENT_SHOULD_FOLLOW" == "true" ]]; then
    FOLLOW_TRIGGER_REASON="leader_new_open"
  fi
  if [[ "$FOLLOWER_CURRENT_ASSET_HELD" == "true" ]]; then
    if [[ -n "$FOLLOW_TRIGGER_REASON" ]]; then
      FOLLOW_TRIGGER_REASON="${FOLLOW_TRIGGER_REASON}+follower_holds_asset"
    else
      FOLLOW_TRIGGER_REASON="follower_holds_asset"
    fi
  fi
  if [[ "$LEADER_EVENT_SHOULD_FOLLOW" == "true" && "$FOLLOWER_CURRENT_ASSET_HELD" != "true" ]]; then
    FOLLOW_IS_FIRST_OPEN="true"
  fi
  if [[ "$FOLLOW_IS_FIRST_OPEN" == "true" && -n "$FOLLOW_SHARES" ]] && decimal_lt "$FOLLOW_SHARES" "$MIN_OPEN_SHARES"; then
    FOLLOW_SHARES="$MIN_OPEN_SHARES"
    FOLLOW_USDC="$(scale_usdc_for_target_shares "${LATEST_ACTIVITY_USDC_SIZE:-0}" "${LATEST_ACTIVITY_SIZE:-0}" "$MIN_OPEN_SHARES")"
    FOLLOW_MIN_OPEN_FLOOR_APPLIED="true"
  elif [[ -n "$FOLLOW_SHARES" ]] && decimal_lt "$FOLLOW_SHARES" "$MIN_COMPATIBLE_SHARES"; then
    FOLLOW_SHARES="$MIN_COMPATIBLE_SHARES"
    FOLLOW_USDC="$(scale_usdc_for_target_shares "${LATEST_ACTIVITY_USDC_SIZE:-0}" "${LATEST_ACTIVITY_SIZE:-0}" "$MIN_COMPATIBLE_SHARES")"
    FOLLOW_MIN_COMPATIBLE_FLOOR_APPLIED="true"
  fi
fi

if [[ "$LATEST_ACTIVITY_TYPE" == "TRADE" && -z "$FOLLOW_TRIGGER_REASON" ]]; then
  echo "skipping because leader/follower gates did not allow follow (leader_status=$LEADER_EVENT_OPEN_GATE_STATUS follower_held=$FOLLOWER_CURRENT_ASSET_HELD)"
  cat > "$SUMMARY" <<EOF_SUMMARY
user=$USER_WALLET
watch_exit=$WATCH_EXIT
positions_gate_exit=$POSITIONS_GATE_EXIT
follower_snapshot_exit=$FOLLOWER_SNAPSHOT_EXIT
submit_exit=not_run
latest_tx=$LATEST_TX
latest_activity_timestamp=$LATEST_ACTIVITY_TIMESTAMP
latest_activity_price=$LATEST_ACTIVITY_PRICE
latest_activity_type=$LATEST_ACTIVITY_TYPE
latest_activity_side=$LATEST_ACTIVITY_SIDE
latest_activity_asset=$LATEST_ACTIVITY_ASSET
latest_activity_size=$LATEST_ACTIVITY_SIZE
latest_activity_usdc_size=$LATEST_ACTIVITY_USDC_SIZE
watch_started_at_unix_ms=$WATCH_STARTED_AT_UNIX_MS
watch_finished_at_unix_ms=$WATCH_FINISHED_AT_UNIX_MS
watch_elapsed_ms=$WATCH_ELAPSED_MS
leader_to_watch_finished_ms=$LEADER_TO_WATCH_FINISHED_MS
follow_share_divisor=$FOLLOW_SHARE_DIVISOR
min_open_shares=$MIN_OPEN_SHARES
min_compatible_shares=$MIN_COMPATIBLE_SHARES
follower_position_check_status=$FOLLOWER_POSITION_CHECK_STATUS
follower_current_asset_net_size=$FOLLOWER_CURRENT_ASSET_NET_SIZE
follower_current_asset_held=$FOLLOWER_CURRENT_ASSET_HELD
follow_is_first_open=$FOLLOW_IS_FIRST_OPEN
follow_min_open_floor_applied=$FOLLOW_MIN_OPEN_FLOOR_APPLIED
follow_min_compatible_floor_applied=$FOLLOW_MIN_COMPATIBLE_FLOOR_APPLIED
follow_trigger_reason=
$(metric_from_positions_gate positions_query_status)
$(metric_from_positions_gate positions_retry_attempts)
$(metric_from_positions_gate current_positions_response_count)
$(metric_from_positions_gate current_event_position_count)
$(metric_from_positions_gate current_event_target_asset_size)
$(metric_from_positions_gate current_event_other_asset_size)
$(metric_from_positions_gate current_event_total_size)
$(metric_from_positions_gate leader_event_open_gate_status)
$(metric_from_positions_gate leader_event_open_gate_reason)
$(metric_from_positions_gate leader_event_should_follow)
latest_activity=$LATEST_ACTIVITY
selected_latest_activity=$SELECTED_ACTIVITY
selected_leader_env=$SELECTED_ENV
account_snapshot_path=$ACCOUNT_SNAPSHOT_FILE
watch_stdout_log=$WATCH_STDOUT
watch_stderr_log=$WATCH_STDERR
positions_stdout_log=$POSITIONS_STDOUT
positions_stderr_log=$POSITIONS_STDERR
account_snapshot_stdout_log=$ACCOUNT_SNAPSHOT_STDOUT
account_snapshot_stderr_log=$ACCOUNT_SNAPSHOT_STDERR
submit_stdout_log=$SUBMIT_STDOUT
submit_stderr_log=$SUBMIT_STDERR
run_dir=$RUN_DIR
status=no_follow_condition_matched
EOF_SUMMARY
  exit 0
fi

ACTION_BIN="$LIVE_SUBMIT_BIN_DEFAULT"
ACTION_ARGS=(
  --root ..
  --selected-leader-env "$SELECTED_ENV"
  --latest-activity "$SELECTED_ACTIVITY"
)
if [[ "$LATEST_ACTIVITY_TYPE" == "MERGE" || "$LATEST_ACTIVITY_TYPE" == "SPLIT" ]]; then
  ACTION_BIN="$CTF_ACTION_BIN_DEFAULT"
  ACTION_ARGS+=(--override-usdc-size "$OPEN_USDC")
  ACTION_ARGS+=(--allow-live-submit)
else
  if [[ -n "$FOLLOW_USDC" ]]; then
    ACTION_ARGS+=(--override-usdc-size "$FOLLOW_USDC")
    ACTION_ARGS+=(--order-type "$FOLLOW_ORDER_TYPE")
  else
    ACTION_ARGS+=(--override-usdc-size "$OPEN_USDC")
  fi
  ACTION_ARGS+=(--allow-live-submit --force-live-submit)
fi

set +e
run_with_shell_fallback "$ACTION_BIN" "${ACTION_ARGS[@]}" >"$SUBMIT_STDOUT" 2>"$SUBMIT_STDERR"
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

remove_watch_seen_tx() {
  local tx="$1"
  [[ -n "$tx" ]] || return 0
  [[ -f "$WATCH_SEEN_TX_FILE" ]] || return 0
  perl -e '
    my ($path, $tx) = @ARGV;
    open my $fh, "<", $path or exit 0;
    my @lines = grep { defined($_) && $_ ne "" } map { chomp; $_ } <$fh>;
    close $fh;
    @lines = grep { $_ ne $tx } @lines;
    open my $out, ">", $path or exit 1;
    for my $line (@lines) { print {$out} "$line\n" if length $line; }
    close $out;
  ' "$WATCH_SEEN_TX_FILE" "$tx" 2>/dev/null || true
}

if [[ "$SUBMIT_EXIT" -eq 0 && -n "$LATEST_TX" ]] && submit_marks_seen; then
  printf '%s\n' "$LATEST_TX" > "$LAST_SUBMITTED_TX_FILE"
elif [[ -n "$LATEST_TX" ]]; then
  remove_watch_seen_tx "$LATEST_TX"
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
positions_gate_exit=$POSITIONS_GATE_EXIT
submit_exit=$SUBMIT_EXIT
latest_tx=$LATEST_TX
latest_activity_timestamp=$LATEST_ACTIVITY_TIMESTAMP
latest_activity_price=$LATEST_ACTIVITY_PRICE
latest_activity_type=$LATEST_ACTIVITY_TYPE
latest_activity_side=$LATEST_ACTIVITY_SIDE
latest_activity_asset=$LATEST_ACTIVITY_ASSET
latest_activity_size=$LATEST_ACTIVITY_SIZE
latest_activity_usdc_size=$LATEST_ACTIVITY_USDC_SIZE
watch_started_at_unix_ms=$WATCH_STARTED_AT_UNIX_MS
watch_finished_at_unix_ms=$WATCH_FINISHED_AT_UNIX_MS
watch_elapsed_ms=$WATCH_ELAPSED_MS
leader_to_watch_finished_ms=$LEADER_TO_WATCH_FINISHED_MS
follow_share_divisor=$FOLLOW_SHARE_DIVISOR
follow_share_size=$FOLLOW_SHARES
follow_usdc_size=$FOLLOW_USDC
min_open_shares=$MIN_OPEN_SHARES
min_compatible_shares=$MIN_COMPATIBLE_SHARES
follower_snapshot_exit=$FOLLOWER_SNAPSHOT_EXIT
follower_position_check_status=$FOLLOWER_POSITION_CHECK_STATUS
follower_current_asset_net_size=$FOLLOWER_CURRENT_ASSET_NET_SIZE
follower_current_asset_held=$FOLLOWER_CURRENT_ASSET_HELD
follow_is_first_open=$FOLLOW_IS_FIRST_OPEN
follow_min_open_floor_applied=$FOLLOW_MIN_OPEN_FLOOR_APPLIED
follow_min_compatible_floor_applied=$FOLLOW_MIN_COMPATIBLE_FLOOR_APPLIED
follow_trigger_reason=$FOLLOW_TRIGGER_REASON
$(metric_from_positions_gate positions_query_status)
$(metric_from_positions_gate positions_retry_attempts)
$(metric_from_positions_gate current_positions_response_count)
$(metric_from_positions_gate current_event_position_count)
$(metric_from_positions_gate current_event_target_asset_size)
$(metric_from_positions_gate current_event_other_asset_size)
$(metric_from_positions_gate current_event_total_size)
$(metric_from_positions_gate leader_event_open_gate_status)
$(metric_from_positions_gate leader_event_open_gate_reason)
$(metric_from_positions_gate leader_event_should_follow)
latest_activity=$LATEST_ACTIVITY
selected_latest_activity=$SELECTED_ACTIVITY
selected_leader_env=$SELECTED_ENV
account_snapshot_path=$ACCOUNT_SNAPSHOT_FILE
watch_stdout_log=$WATCH_STDOUT
watch_stderr_log=$WATCH_STDERR
positions_stdout_log=$POSITIONS_STDOUT
positions_stderr_log=$POSITIONS_STDERR
account_snapshot_stdout_log=$ACCOUNT_SNAPSHOT_STDOUT
account_snapshot_stderr_log=$ACCOUNT_SNAPSHOT_STDERR
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
$(metric_from_submit order_execution_style)
$(metric_from_submit order_size)
$(metric_from_submit order_usdc_size)
$(metric_from_submit submit_order_id)
$(metric_from_submit submit_order_status)
$(metric_from_submit submit_success)
$(metric_from_submit submit_transaction_hashes)
$(metric_from_submit submit_trade_ids)
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
