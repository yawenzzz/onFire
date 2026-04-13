# polymarket-arb

Initial scaffold for a fail-closed structural arbitrage system.

Current posture: NO_TRADE by default until surface/compliance gates are explicitly resolved.

## Development Quickstart

Run tests:

```bash
make test
```

Run a minimal shadow demo from an input file:

```bash
PYTHONPATH=polymarket_arb python -m polymarket_arb.app.entrypoint --input-file examples/shadow-input.json --output ./demo-report.json
```

## Implemented Features

- fail-closed venue / compliance / market-state gates
- structural template parsing and grouping
- parity / directional / deterministic cross-market model primitives
- shadow metrics, certification report, dashboard payload, archive bundle
- real public gateway client for events and market book samples
- json/jsonl capture, replay, and capture-to-shadow bundle flow
- CLI and script entrypoints for demo, capture, and archive flows

## Manual Verification

- Run full tests: `make test`
- Run shadow demo: `make shadow-demo`
- Run capture/archive demo: `bash scripts/run_capture_archive_shadow.sh`
- Inspect generated bundle under `.omx/shadow-archives/sessions/demo-s1/`
- Inspect real public samples under `examples/live/`

## Current Safety Posture

- Prototype is shadow-first and fail-closed.
- Live trading is intentionally not enabled.
- Real trading remains blocked until exact venue/product surface and geographic/compliance eligibility are explicitly resolved.
