#!/usr/bin/env bash
set -euo pipefail

: "${POLYMARKET_KEY_ID:?missing POLYMARKET_KEY_ID}"
: "${POLYMARKET_SECRET_KEY:?missing POLYMARKET_SECRET_KEY}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PM_PATH="${PM_PATH:-${1:-/v1/ws/markets}}"
PM_METHOD="${PM_METHOD:-GET}"
PM_TIMESTAMP_OVERRIDE="${PM_TIMESTAMP_OVERRIDE:-}"
CLI_ARGS=(
  --access-key "$POLYMARKET_KEY_ID"
  --private-key "$POLYMARKET_SECRET_KEY"
  --path "$PM_PATH"
  --method "$PM_METHOD"
)

if [ -n "$PM_TIMESTAMP_OVERRIDE" ]; then
  CLI_ARGS+=(--timestamp "$PM_TIMESTAMP_OVERRIDE")
fi

while IFS='=' read -r key value; do
  export "$key=$value"
done < <(
  cd "$ROOT_DIR"
  PYTHONPATH=polymarket_arb python3 -m polymarket_arb.auth.pm_auth_cli "${CLI_ARGS[@]}"
)

echo "Generated PM_* exports for $PM_PATH at $PM_TIMESTAMP"
echo "Use immediately; Polymarket timestamps must be fresh."
