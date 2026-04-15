# rust-copytrader scaffold

This crate is the Rust-side, contract-first scaffold for the copy-trading PRD.
It locks the non-negotiable execution posture before any real venue I/O is added:

- fixed hot-path order: `activity -> positions -> market websocket -> submit -> verification`
- fail-closed latency posture with a hard reject above 200ms to submit
- explicit live-mode gate while third-party leader activity listening remains unverified
- single-node, local-file-only runtime evidence with JSONL append helpers and session snapshot path helpers (not yet full session-owned persistence)

## Current implementation status

### Lane 1: preview + submit contract skeletons
- `src/pipeline/orchestrator.rs` wires the ordered hot path through reconciliation, quote checks, budget enforcement, preview gating, submit lifecycle, and verification outcome application.
- `src/execution/pre_trade_gate.rs` enforces fail-closed market-open, price-shape, quantity, and remaining-budget checks before a submit-ready `OrderIntent` can exist.
- `src/adapters/order_api.rs` defines the submit-facing order contract currently shared by replay/orchestrator flows.

### Lane 2: snapshots + telemetry in the runtime session
- `src/app.rs` provides `RuntimeSession`, which ties bootstrap state, orchestrator results, runtime metrics, latency accounting, and latest snapshot generation together.
- `src/pipeline/trace_context.rs` records ordered stage timestamps so latency reporting stays aligned with the mandated execution sequence.
- `src/persistence/snapshots.rs` defines a stable local JSON snapshot shape and deterministic session-root path helper for operator/runtime evidence.
- `src/persistence/jsonl.rs` provides the append-only file primitive already used by capture-oriented tests, but it is not yet wired into `RuntimeSession`.
- `src/telemetry/metrics.rs` and `src/telemetry/latency.rs` accumulate submit/reject/timeout counters plus per-stage timing deltas in memory for the current process.

### Lane 3: transport boundaries for future live adapters
- `src/adapters/activity.rs` keeps `live_listen`, `shadow_poll`, and `replay` mode selection explicit and fail-closed.
- `src/adapters/positions.rs`, `src/adapters/market_ws.rs`, and `src/adapters/verification.rs` define the current boundary contracts for positions reconciliation, market quote validation, and post-submit verification correlation.
- `src/replay/harness.rs` preserves replay parity against the same stage ordering and budget posture the eventual live transports must respect.

## Test coverage

The scaffold is intentionally contract-first. Key coverage includes:

- `tests/activity_adapter.rs` / `tests/bootstrap_mode.rs` — live-mode feasibility gate and blocked/shadow/replay decisions
- `tests/pre_trade_gate.rs` / `tests/orchestrator.rs` — preview+submit contract skeletons, pre-submit fail-closed checks, and lifecycle outcomes
- `tests/runtime_session.rs` / `tests/snapshots.rs` / `tests/telemetry_latency.rs` — runtime session evidence, stable snapshot shape, and stage-latency accounting
- `tests/e2e_replay.rs` / `tests/perf_budget.rs` — replay parity, fixed stage ordering, and hard budget rejection behavior
- `tests/reconciliation_and_market_ws.rs` / `tests/verification_adapter.rs` / `tests/verification_state.rs` — stale data rejection, verification correlation, timeout handling, and state-machine separation

Run the crate verification with:

```bash
cd rust-copytrader
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## What is still intentionally missing

This crate is still a scaffold, not a production trading runtime. Remaining work includes:

1. concrete HTTP/router/auth implementations behind the preview + submit contract
2. session-level file writing/rotation that persists `RuntimeSession` snapshots beyond in-memory accumulation
3. external metrics export and richer operator-facing reporting surfaces
4. concrete live/replay transport integrations that feed the adapter boundaries without breaking replay parity
5. a real CLI/runtime entrypoint; `src/main.rs` is still a bootstrap stub rather than an operator-facing command surface

## Review notes

The current implementation is small, readable, and strongly fail-closed. The main remaining gap is breadth, not the correctness of the encoded contracts:

- live-mode safety is explicit and remains blocked by default
- hot-path budget enforcement is encoded before submit, not merely documented
- submit failure and post-submit verification failure remain separate states
- replay coverage already protects the fixed activity -> positions -> market websocket -> submit -> verification sequence
- current runtime evidence is still process-local (`RuntimeSession::snapshot`, `RuntimeMetrics`, `LatencyReport`) until file/report wiring lands

Future lanes should extend these existing boundaries rather than bypass them.
