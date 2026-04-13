#!/usr/bin/env bash
set -euo pipefail
MARKET_ID=$(python3 - <<'PY'
import json
from pathlib import Path
obj = json.loads(Path('examples/live/events-sample.json').read_text())
print(obj['events'][0]['markets'][0]['slug'])
PY
)
PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.realtime_probe_cli \
  --ws-url wss://api.polymarket.us/v1/ws/markets \
  --market-ids "$MARKET_ID" \
  --limit 1 \
  --output realtime-probe.json
