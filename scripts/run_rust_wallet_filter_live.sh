#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT_DIR/rust-copytrader"
DISCOVERY_DIR="$ROOT_DIR/.omx/discovery"
CATEGORIES="SPECIALIST"
LIMIT=25
TOP=5
INTERVAL_SEC=60
CONNECT_TIMEOUT_MS=8000
MAX_TIME_MS=20000
PROXY="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"
ONCE=0
NO_CLEAR=0
SCAN_BIN=""
RENDER_ONLY=0

usage() {
  cat <<USAGE
usage: bash scripts/run_rust_wallet_filter_live.sh [options]

options:
  --categories <SPECIALIST|CSV>   categories to scan (default: SPECIALIST)
  --limit <n>                     leaderboard fetch limit per category (default: 25)
  --top <n>                       candidate rows to show per category (default: 5)
  --interval-sec <n>              refresh interval in loop mode (default: 60)
  --proxy <url>                   http proxy for data-api/gamma-api
  --discovery-dir <path>          discovery dir (default: .omx/discovery)
  --connect-timeout-ms <n>        curl connect timeout (default: 8000)
  --max-time-ms <n>               curl max time (default: 20000)
  --once                          run one scan and exit
  --scan-bin <path>                prebuilt scan_copy_leader_categories binary
  --render-only                    do not rescan; render existing discovery artifacts
  --no-clear                      do not clear terminal between refreshes
  -h, --help                      show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --categories) CATEGORIES="$2"; shift 2 ;;
    --limit) LIMIT="$2"; shift 2 ;;
    --top) TOP="$2"; shift 2 ;;
    --interval-sec) INTERVAL_SEC="$2"; shift 2 ;;
    --proxy) PROXY="$2"; shift 2 ;;
    --discovery-dir) DISCOVERY_DIR="$2"; shift 2 ;;
    --connect-timeout-ms) CONNECT_TIMEOUT_MS="$2"; shift 2 ;;
    --max-time-ms) MAX_TIME_MS="$2"; shift 2 ;;
    --scan-bin) SCAN_BIN="$2"; shift 2 ;;
    --render-only) RENDER_ONLY=1; shift ;;
    --once) ONCE=1; shift ;;
    --no-clear) NO_CLEAR=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$DISCOVERY_DIR"
LAST_LOG="$DISCOVERY_DIR/wallet-filter-live.last.log"
SUMMARY_PATH="$DISCOVERY_DIR/wallet-filter-v1-summary.txt"

render_snapshot() {
  python3 - "$DISCOVERY_DIR" "$SUMMARY_PATH" "$TOP" "$CATEGORIES" "$LIMIT" "$PROXY" "$LAST_LOG" <<'PY'
import datetime as dt
import glob
import os
import sys
from pathlib import Path


def parse_report(path: Path):
    top = {}
    candidates = []
    current = None
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith('== candidate ') and line.endswith(' =='):
            current = {'candidate_index': line[len('== candidate '):-len(' ==')]}
            candidates.append(current)
            continue
        if '=' not in line:
            continue
        key, value = line.split('=', 1)
        if current is None:
            top[key] = value
        else:
            current[key] = value
    return top, candidates


def parse_summary(path: Path):
    if not path.exists():
        return {}, []
    top = {}
    categories = []
    current = None
    for raw in path.read_text().splitlines():
        line = raw.rstrip('\n')
        if not line:
            continue
        if line.startswith('== category ') and line.endswith(' =='):
            name = line[len('== category '):-len(' ==')]
            current = {'category': name, 'body': []}
            categories.append(current)
            continue
        if current is None and '=' in line:
            key, value = line.split('=', 1)
            top[key] = value
        elif current is not None:
            current['body'].append(line)
            if '=' in line:
                key, value = line.split('=', 1)
                current[key] = value
    return top, categories


def short_wallet(value: str) -> str:
    if not value or value in {'none', 'unknown'}:
        return value
    if len(value) <= 14:
        return value
    return value[:8] + '…' + value[-4:]


def fmt_ratio(value: str) -> str:
    try:
        return f"{float(value)*100:.1f}%"
    except Exception:
        return value


def pad(text: str, width: int) -> str:
    text = str(text)
    if len(text) > width:
        return text[:width-1] + '…'
    return text.ljust(width)


def candidate_sort_key(c):
    try:
        score = int(c.get('score_total', '-999999'))
    except Exception:
        score = -999999
    reasons = c.get('rejection_reasons', 'none')
    reason_count = 0 if reasons == 'none' else len([x for x in reasons.split(',') if x])
    try:
        month_rank = int(c.get('month_rank', '999999'))
    except Exception:
        month_rank = 999999
    return (-score, reason_count, month_rank)


discovery_dir = Path(sys.argv[1])
summary_path = Path(sys.argv[2])
top_n = int(sys.argv[3])
categories_arg = sys.argv[4]
limit = sys.argv[5]
proxy = sys.argv[6]
last_log = Path(sys.argv[7])

summary_top, summary_categories = parse_summary(summary_path)
now = dt.datetime.now().astimezone().strftime('%Y-%m-%d %H:%M:%S %Z')

print(f"{now} wallet_filter_v1 live scan  categories={categories_arg} limit={limit}")
print(f"proxy={proxy}")
if summary_top:
    print(
        "summary "
        f"scanned={summary_top.get('categories_scanned','?')} "
        f"passed={summary_top.get('categories_passed','?')} "
        f"rejected={summary_top.get('categories_rejected','?')} "
        f"best_pass={summary_top.get('best_pass_category','none')}:{short_wallet(summary_top.get('best_pass_wallet','none'))} "
        f"best_rejected={summary_top.get('best_rejected_category','none')}:{short_wallet(summary_top.get('best_rejected_wallet','none'))} "
        f"watchlist={summary_top.get('watchlist_candidates','none')}"
    )
else:
    print("summary missing")
print()

for category_info in summary_categories:
    category = category_info['category']
    report_path = discovery_dir / f"wallet-filter-v1-{category.lower()}.txt"
    if not report_path.exists():
        print(f"[{category}] report missing: {report_path}")
        print()
        continue
    top, candidates = parse_report(report_path)
    candidates = sorted(candidates, key=candidate_sort_key)[:top_n]
    selected_wallet = top.get('selected_wallet', 'none')
    print(
        f"[{category}] status={category_info.get('status', 'unknown')} "
        f"candidate_count={top.get('candidate_count', '?')} "
        f"selected={short_wallet(selected_wallet)} score={top.get('selected_score', 'none')}"
    )
    header = (
        pad('#', 2) + ' ' +
        pad('status', 9) + ' ' +
        pad('wallet', 15) + ' ' +
        pad('score', 5) + ' ' +
        pad('review', 9) + ' ' +
        pad('maker', 5) + ' ' +
        pad('flip60', 7) + ' ' +
        pad('tail24', 7) + ' ' +
        pad('tail72', 7) + ' ' +
        pad('copy', 7) + ' ' +
        pad('uniq', 4) + ' ' +
        'reasons'
    )
    print(header)
    for c in candidates:
        print(
            pad(c.get('candidate_index', '?'), 2) + ' ' +
            pad(c.get('status', 'unknown'), 9) + ' ' +
            pad(short_wallet(c.get('wallet', 'none')), 15) + ' ' +
            pad(c.get('score_total', 'na'), 5) + ' ' +
            pad(c.get('review_status', 'na'), 9) + ' ' +
            pad(c.get('maker_rebate_count', 'na'), 5) + ' ' +
            pad(c.get('flip60', 'na'), 7) + ' ' +
            pad(fmt_ratio(c.get('tail24', 'na')), 7) + ' ' +
            pad(fmt_ratio(c.get('tail72', 'na')), 7) + ' ' +
            pad(fmt_ratio(c.get('copyable_ratio', 'na')), 7) + ' ' +
            pad(c.get('unique_markets_90d', 'na'), 4) + ' ' +
            c.get('rejection_reasons', 'none')
        )
    print()

if last_log.exists():
    log_tail = last_log.read_text().strip().splitlines()[-6:]
    if log_tail:
        print('last_scan_log:')
        for line in log_tail:
            print(line)
PY
}

ensure_scan_bin() {
  if [[ -n "$SCAN_BIN" && -x "$SCAN_BIN" ]]; then
    return 0
  fi
  if [[ -z "$SCAN_BIN" ]]; then
    SCAN_BIN="$RUST_DIR/target/debug/scan_copy_leader_categories"
  fi
  if [[ -x "$SCAN_BIN" ]]; then
    return 0
  fi
  pushd "$RUST_DIR" >/dev/null
  cargo build --quiet --bin scan_copy_leader_categories >>"$LAST_LOG" 2>&1
  popd >/dev/null
}

run_once() {
  local status=0
  if [[ "$RENDER_ONLY" -eq 0 ]]; then
    : >"$LAST_LOG"
    ensure_scan_bin
    set +e
    "$SCAN_BIN" \
      --discovery-dir "$DISCOVERY_DIR" \
      --categories "$CATEGORIES" \
      --limit "$LIMIT" \
      --proxy "$PROXY" \
      --connect-timeout-ms "$CONNECT_TIMEOUT_MS" \
      --max-time-ms "$MAX_TIME_MS" \
      >"$LAST_LOG" 2>&1
    status=$?
    set -e
  fi
  if [[ "$NO_CLEAR" -eq 0 ]]; then
    printf '\033[2J\033[H'
  fi
  render_snapshot
  echo
  echo "scan_exit_code=$status  summary_path=$SUMMARY_PATH"
  return 0
}

while true; do
  run_once
  if [[ "$ONCE" -eq 1 ]]; then
    break
  fi
  sleep "$INTERVAL_SEC"
done
