# rust-copytrader

Minimal Rust scaffold for the copy-trading hot path defined in `.omx/plans/prd-trace-strat-copytrading-rust.md`.

## Current implemented core

- Fixed deterministic path: `activity -> positions -> market websocket -> submit -> verification`
- Hard fail-closed budget enforcement at `>200ms`
- Explicit live-mode gate for unsupported / unverified leader-activity listening
- Replay fixture types plus deterministic replay harness for regression coverage
- Pre-trade gating for market-open state, valid price shape, positive size, and remaining-budget checks
- Local-file append-only persistence helper (`src/persistence/jsonl.rs`)

## Deliberate non-goals in this scaffold

- No database or distributed coordination
- No automatic strategy optimization
- No unlocked live leader-listen mode without external capability proof

## Main modules

- `src/pipeline/orchestrator.rs` — deterministic hot-path orchestration
- `src/execution/pre_trade_gate.rs` — submit readiness checks
- `src/replay/fixture.rs` — replay scenario contracts
- `src/replay/harness.rs` — deterministic replay runner
- `tests/e2e_replay.rs` — end-to-end replay regression coverage
- `tests/pre_trade_gate.rs` — guardrail coverage for submit gating
