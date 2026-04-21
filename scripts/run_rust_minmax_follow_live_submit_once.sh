#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORCE_FOLLOW_ONCE_BIN="${FORCE_FOLLOW_ONCE_BIN:-$ROOT/scripts/run_rust_follow_last_action_force_live_once.sh}"

cd "$ROOT"
REQUIRE_NEW_ACTIVITY=1 bash "$FORCE_FOLLOW_ONCE_BIN" "$@"
