#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_URL="${REPO_URL:-https://github.com/yawenzzz/onFire.git}"
TARGET_DIR="${TARGET_DIR:-$HOME/onFire}"
BRANCH="${BRANCH:-main}"
PROXY="${POLYMARKET_CURL_PROXY:-http://127.0.0.1:7897}"
MONITOR_COLUMNS="${MONITOR_COLUMNS:-140}"
MODE="${1:-print}"

usage() {
  cat <<USAGE
usage: bash scripts/bootstrap_copytrader_on_new_machine.sh [print|run]

modes:
  print   print the exact setup commands (default)
  run     execute the setup locally on this machine

env overrides:
  REPO_URL=https://github.com/yawenzzz/onFire.git
  TARGET_DIR=$HOME/onFire
  BRANCH=main
  POLYMARKET_CURL_PROXY=http://127.0.0.1:7897
  MONITOR_COLUMNS=140
USAGE
}

render_commands() {
  cat <<CMDS
set -euo pipefail

if ! command -v git >/dev/null 2>&1; then
  echo "missing git" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "missing cargo/rustup; install rust first" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "missing python3" >&2
  exit 1
fi

if [ ! -d "$TARGET_DIR/.git" ]; then
  git clone "$REPO_URL" "$TARGET_DIR"
fi

cd "$TARGET_DIR"
git fetch origin
git checkout "$BRANCH"
git pull --ff-only origin "$BRANCH"

export POLYMARKET_CURL_PROXY="$PROXY"
mkdir -p .omx/discovery .omx/monitor .omx/position-targeting

cd rust-copytrader
cargo build --bin discover_copy_leader \
            --bin scan_copy_leader_categories \
            --bin run_copytrader_monitor_v1 \
            --bin select_copy_leader \
            --bin watch_copy_leader_activity \
            --bin run_position_targeting_demo
cd ..

bash scripts/run_rust_wallet_filter_live.sh --once --categories SPECIALIST --limit 25 --top 5

cd rust-copytrader
cargo run --bin discover_copy_leader -- \
  --discovery-dir ../.omx/discovery \
  --proxy "$PROXY" \
  --category SPECIALIST \
  --connect-timeout-ms 8000 \
  --max-time-ms 20000
cd ..

python3 scripts/rank_wallet_stability.py \
  --discovery-dir .omx/discovery \
  --out .omx/discovery/wallet-stability-top10.txt

MONITOR_COLUMNS="$MONITOR_COLUMNS" bash scripts/run_rust_monitor_v2.sh
CMDS
}

case "$MODE" in
  print)
    render_commands
    ;;
  run)
    tmpfile="$(mktemp)"
    render_commands > "$tmpfile"
    bash "$tmpfile"
    rm -f "$tmpfile"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "unknown mode: $MODE" >&2
    usage >&2
    exit 2
    ;;
esac
