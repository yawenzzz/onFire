#!/usr/bin/env bash
set -euo pipefail
CAPTURE_FILE=examples/live/capture.jsonl
ARCHIVE_ROOT=.omx/shadow-archives
printf '{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n' > "$CAPTURE_FILE"
PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.capture_bundle_cli \
  --capture-file "$CAPTURE_FILE" \
  --archive-root "$ARCHIVE_ROOT" \
  --session-id demo-s1 \
  --surface-id polymarket-us
ls "$ARCHIVE_ROOT"/sessions/demo-s1/certification-report.json >/dev/null
