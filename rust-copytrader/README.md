# rust-copytrader scaffold

This crate is the Rust-side scaffold for the PRD in `.omx/plans/prd-trace-strat-copytrading-rust.md`.
Its current focus is to lock the non-negotiable execution contracts before live venue wiring:

- fixed hot-path order: `activity -> positions -> market websocket -> submit -> verification`
- fail-closed latency posture with a hard reject above 200ms to submit
- explicit live-mode gate while third-party leader activity listening remains unverified
- single-node, local-file-only persistence via append-only JSONL

## What exists today

### Bootstrap and live-mode gating
- `src/config.rs` models `ActivityMode` plus the live-mode unlock checklist.
- `src/app.rs` now provides both the bootstrap decision and a `RuntimeSession` that ties together bootstrap state, orchestrator results, telemetry, and snapshot generation.
- `src/adapters/activity.rs` keeps `live_listen`, `shadow_poll`, and `replay` explicit so unsupported live listening cannot activate implicitly.

### Hot-path contracts already encoded
- `src/adapters/positions.rs` rejects stale or mismatched `/positions` snapshots and emits a net delta only when the leader position actually changes.
- `src/adapters/market_ws.rs` enforces fresh quotes and rejects invalid spreads before downstream execution.
- `src/domain/budget.rs` models the hard latency budget and per-stage schedulability checks.
- `src/pipeline/trace_context.rs` records ordered stage timestamps for later latency accounting.

### Post-submit state, local persistence, and observability
- `src/execution/state_machine.rs` distinguishes submit failure from post-submit verification outcomes.
- `src/adapters/verification.rs` correlates own-order verification events, explicit mismatches, and timeout-driven fail-closed outcomes.
- `src/persistence/jsonl.rs` provides append-only local durability aligned with the no-database constraint.
- `src/persistence/snapshots.rs` renders stable local runtime snapshots for leader state and runtime gate state.
- `src/telemetry/metrics.rs` and `src/telemetry/latency.rs` track reject counters, submit counts, timeout counts, and stage-latency deltas from trace context.

## Test coverage in the scaffold

The current integration tests are intentionally contract-first:

- `tests/activity_adapter.rs` — live-mode feasibility gate and normalized activity payloads
- `tests/bootstrap_mode.rs` — bootstrap decisions across blocked, shadow, and unlocked live modes
- `tests/runtime_session.rs` — end-to-end session updates from replay processing into snapshots and telemetry
- `tests/hot_path_budget.rs` — hard latency budget, freshness checks, trace timing, JSONL durability
- `tests/reconciliation_and_market_ws.rs` — stale snapshot rejection, no-net-change handling, fresh/stale quote validation
- `tests/verification_state.rs` — separation between submit failure, verification pending, and terminal verification outcomes
- `tests/verification_adapter.rs` — correlation checks, explicit mismatch rejection, and timeout-driven verification transitions
- `tests/snapshots.rs` / `tests/telemetry_latency.rs` — stable local snapshot rendering and per-stage telemetry accounting

Run them with:

```bash
cd rust-copytrader
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Current limitations / next implementation steps

This scaffold does **not** yet provide a complete live trading pipeline. The following remain to be wired by the implementation lanes before the PRD can be considered complete:

1. auth/router wiring for the real preview + submit path
2. market-state caches / leader-state caches and session-level persistence rotation
3. external metrics export and richer operator-facing snapshot/report surfaces
4. integration of real activity, positions, market-ws, and verification adapters against live or replayed transports

## Review notes

The current scaffold is clean, focused, and already enforces the most important fail-closed rules. The main remaining gap is not correctness of the implemented pieces, but breadth: several PRD modules are still represented only by tests or by placeholder module boundaries. That is acceptable for a scaffold, but it should stay clearly documented so downstream workers do not mistake this crate for a feature-complete trading runtime.
