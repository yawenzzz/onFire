#!/usr/bin/env bash
set -euo pipefail

: "${PRIVATE_KEY:?missing PRIVATE_KEY}"

ENV_PATH="${ENV_PATH:-.env.local}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_JSON="$(mktemp)"

cleanup() {
  rm -f "$TMP_JSON"
}

trap cleanup EXIT

if [ ! -f "$ROOT_DIR/$ENV_PATH" ] && [ -f "$ROOT_DIR/.env.local.example" ]; then
  cp "$ROOT_DIR/.env.local.example" "$ROOT_DIR/$ENV_PATH"
fi

(
  cd "$ROOT_DIR"
  python3 scripts/derive_clob_creds_python_template.py > "$TMP_JSON"
  PYTHONPATH=polymarket_arb python3 - "$ROOT_DIR/$ENV_PATH" "$TMP_JSON" <<'PY'
import json
import sys
from pathlib import Path

from polymarket_arb.auth.local_env_writer import upsert_env_file

env_path = Path(sys.argv[1])
creds_path = Path(sys.argv[2])
payload = json.loads(creds_path.read_text())

upsert_env_file(
    env_path,
    {
        "CLOB_API_KEY": payload["CLOB_API_KEY"],
        "CLOB_SECRET": payload["CLOB_SECRET"],
        "CLOB_PASS_PHRASE": payload["CLOB_PASS_PHRASE"],
    },
)
PY
)

echo "Wrote CLOB credentials to $ENV_PATH"
echo "Verifying $ENV_PATH via scripts/check_secrets.sh"
(
  cd "$ROOT_DIR"
  set -a
  source "$ENV_PATH"
  set +a
  bash scripts/check_secrets.sh
)
echo "Load into current shell with: set -a; source $ENV_PATH; set +a"
