#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
WATCH_BIN_DEFAULT="$ROOT/scripts/run_rust_watch_copy_leader_activity_ws.sh" \
bash "$ROOT/scripts/run_rust_minmax_follow_live.sh" "$@"
