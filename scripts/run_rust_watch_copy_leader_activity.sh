#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_BIN="${CARGO_BIN:-cargo}"

cd "$ROOT/rust-copytrader"
"$CARGO_BIN" run --bin watch_copy_leader_activity -- "$@"
