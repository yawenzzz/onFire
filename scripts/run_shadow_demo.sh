#!/usr/bin/env bash
set -euo pipefail
PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.entrypoint --input-file examples/shadow-input.json --output demo-report.json
