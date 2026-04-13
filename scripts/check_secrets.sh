#!/usr/bin/env bash
set -euo pipefail
for key in PM_ACCESS_KEY PM_SIGNATURE PM_TIMESTAMP CLOB_API_KEY CLOB_SECRET CLOB_PASS_PHRASE; do
  if [ -n "${!key-}" ]; then
    echo "$key=true"
  else
    echo "$key=false"
  fi
done
