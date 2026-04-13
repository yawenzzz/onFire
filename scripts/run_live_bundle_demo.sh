#!/usr/bin/env bash
set -euo pipefail
PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.live_bundle_cli \
  --archive-root .omx/live-shadow-archives \
  --capture-output .omx/live-shadow-archives/capture.jsonl \
  --session-id live-demo-s1 \
  --surface-id polymarket-us \
  --demo \
  --limit 1
