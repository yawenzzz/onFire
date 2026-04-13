#!/usr/bin/env bash
set -euo pipefail
CAPTURE_FILE=examples/live/capture-demo.jsonl
printf '{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n' > "$CAPTURE_FILE"
PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.entrypoint --input-file examples/shadow-input.json --output demo-report.json
printf 'capture jsonl: %s\n' "$CAPTURE_FILE"
