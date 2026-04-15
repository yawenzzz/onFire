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
- `src/adapters/http_submit.rs` now builds authenticated `POST /orders` HTTP request specs with explicit L2 header requirements so the scaffold has a concrete REST submission contract even before live network execution is wired.
- `src/adapters/http_submit.rs` also provides a curl-based execution command spec plus runner abstraction, so the scaffold can now test how a real authenticated submit would be executed without performing live network calls by default.
- `src/adapters/signing.rs` now bridges auth material, signer output, and signed order envelopes so authenticated request headers and signed order payloads can be prepared from one explicit contract.
- `src/adapters/submit_pipeline.rs` now composes auth material validation, signing, authenticated request building, and command-runner execution into one end-to-end submit pipeline contract.

### Lane 2: snapshots + telemetry in the runtime session
- `src/app.rs` provides `RuntimeSession` plus `RuntimeSessionRecorder`, tying bootstrap state, orchestrator results, runtime metrics, latency accounting, snapshot persistence, rotating local logs, and operator report generation together.
- `src/pipeline/trace_context.rs` records ordered stage timestamps so latency reporting stays aligned with the mandated execution sequence.
- `src/persistence/snapshots.rs` defines a stable local JSON snapshot shape plus retained session snapshot archives for operator/runtime evidence.
- `src/persistence/jsonl.rs` rotates append-only activity/order/verification logs under the local session root without introducing a database.
- `src/telemetry/metrics.rs`, `src/telemetry/latency.rs`, and `src/telemetry/report.rs` accumulate submit/reject/timeout counters, per-stage timing deltas, and operator-facing JSON/text report surfaces.

### Lane 3: transport boundaries for future live adapters
- `src/adapters/activity.rs` keeps `live_listen`, `shadow_poll`, and `replay` mode selection explicit and fail-closed.
- `src/adapters/transport.rs` now resolves replay/shadow/live transport skeletons from a `TransportBoundaryConfig`, rejects mixed boundary modes fail-closed, and keeps the live-mode gate plus replay parity intact across activity, positions, market quote, and verification frames.
- `src/adapters/positions.rs`, `src/adapters/market_ws.rs`, and `src/adapters/verification.rs` define the current boundary contracts for positions reconciliation, market quote validation, and post-submit verification correlation.
- `src/replay/harness.rs` preserves replay parity against the same stage ordering and budget posture the eventual live transports must respect.

## Test coverage

The scaffold is intentionally contract-first. Key coverage includes:

- `tests/activity_adapter.rs` / `tests/bootstrap_mode.rs` — live-mode feasibility gate and blocked/shadow/replay decisions
- `tests/pre_trade_gate.rs` / `tests/orchestrator.rs` — preview+submit contract skeletons, pre-submit fail-closed checks, and lifecycle outcomes
- `tests/http_submit_contract.rs` — authenticated `/orders` request-spec generation, missing-header rejection, and auth-readiness rejection
- `tests/http_submit_executor.rs` — curl command generation plus execution-runner success/failure behavior
- `tests/signing_contract.rs` — auth-material validation, signer bridging, and deriving L2 headers from explicit signing inputs
- `tests/submit_pipeline.rs` — end-to-end composition from auth material + unsigned order to executed submit command output
- `tests/runtime_session.rs` / `tests/session_persistence.rs` / `tests/snapshots.rs` / `tests/telemetry_latency.rs` — runtime session evidence, rotating local persistence, stable snapshot shape, and stage-latency accounting
- `tests/e2e_replay.rs` / `tests/perf_budget.rs` / `tests/transport_runtime.rs` — replay parity, fixed stage ordering, hard budget rejection behavior, config-driven transport selection, mixed-mode fail-closed behavior, and live-gate enforcement
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

1. real cryptographic signing and live network execution backed by a production HTTP client instead of the new contract/runner abstractions (the end-to-end submit contract now exists, but still uses stub signers and runner abstractions)
2. external metrics export beyond the new local JSON/text operator reports
3. concrete network-backed live/replay transport integrations that feed the config-driven adapter boundaries without breaking replay parity

## Review notes

The current implementation is small, readable, and strongly fail-closed. The main remaining gap is breadth, not the correctness of the encoded contracts:

- live-mode safety is explicit and remains blocked by default
- hot-path budget enforcement is encoded before submit, not merely documented
- submit failure and post-submit verification failure remain separate states
- replay coverage already protects the fixed activity -> positions -> market websocket -> submit -> verification sequence
- current runtime evidence is still process-local (`RuntimeSession::snapshot`, `RuntimeMetrics`, `LatencyReport`) until file/report wiring lands

Future lanes should extend these existing boundaries rather than bypass them.
