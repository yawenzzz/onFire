# Rust Copytrader Scaffold Review

_Date:_ 2026-04-15  
_Scope reviewed:_ `rust-copytrader/`

## Summary

The Rust scaffold is now in better shape than the earlier scaffold review implied. The current crate already covers the core contract slices for the approved PRD items:

- preview + submit-facing contract skeletons with fail-closed budget checks
- deeper session-level snapshots + telemetry wiring
- explicit transport boundaries for future activity/positions/market-ws/verification integrations

This remains a scaffold, but it is no longer “missing an orchestrator” or “missing replay coverage.” Those pieces now exist and are test-backed.

## What is implemented well

### 1. Lane 1 contracts are encoded in runtime code, not only in tests
- `src/pipeline/orchestrator.rs` enforces the fixed stage order and keeps latency rejection ahead of submit.
- `src/execution/pre_trade_gate.rs` blocks invalid market state, price shape, quantity, and exhausted remaining budget before an order intent is emitted.
- `src/execution/state_machine.rs` keeps `submit_failed`, `submitted_unverified`, `verified`, `verification_mismatch`, and `verification_timeout` distinct.

### 2. Lane 2 runtime evidence is wired through one session surface
- `src/app.rs` centralizes bootstrap choice, orchestrator execution, runtime metrics, latency accumulation, and latest snapshot materialization.
- `src/persistence/snapshots.rs` defines a stable JSON shape for local runtime evidence and a deterministic session-root path helper.
- `src/persistence/jsonl.rs` already provides the local append primitive needed for file-backed evidence, which keeps the no-database direction intact.
- `src/telemetry/metrics.rs` and `src/telemetry/latency.rs` keep the runtime accounting small and deterministic.

### 3. Lane 3 transport boundaries preserve replay parity
- `src/adapters/activity.rs` keeps live/shadow/replay selection explicit and fail-closed.
- `src/adapters/positions.rs`, `src/adapters/market_ws.rs`, and `src/adapters/verification.rs` define the current input/output contracts that future live adapters need to satisfy.
- `src/replay/harness.rs` and `tests/e2e_replay.rs` protect the same ordered path expected from eventual live transport wiring.

## Review findings

### Strengths
- The implementation stays intentionally small and readable.
- The 200ms hard ceiling is enforced in code paths, not treated as operator guidance.
- Replay and orchestrator coverage make regressions in stage order or lifecycle status hard to hide.
- The code respects the single-node/local-file-only constraint and avoids premature infrastructure expansion.
- The transport boundary surface in `src/adapters/transport.rs` is already narrow enough that future live adapters can be added without rewriting the replay contract.

### Remaining gaps

These are remaining breadth gaps, not correctness defects in the implemented scaffold:

1. The preview/submit boundary is still an in-process contract; no concrete authenticated venue/router client is wired yet.
2. `RuntimeSession` currently retains/render snapshots in memory, but does not yet own session-level file persistence/rotation.
3. External metrics export and operator-facing reporting remain outside the crate.
4. `src/main.rs` is still a bootstrap stub, so there is no operator-facing CLI/report entrypoint yet.
5. Adapter boundaries are currently concrete module contracts rather than async trait-object integrations; that is acceptable for the scaffold, but future live wiring should preserve replay parity and fail-closed behavior.

## Documentation corrections from this review

- The crate already has local-file evidence primitives (`src/persistence/jsonl.rs` and `session_snapshot_path`), but not end-to-end session-owned snapshot persistence or rotation.
- Current operator/runtime evidence is process-local: `RuntimeSession::snapshot()`, `RuntimeMetrics`, and `LatencyReport` expose the data, but no report/export surface consumes them yet.
- The scaffold is not “missing transport boundaries”; the missing piece is concrete live wiring behind the existing boundaries.

## Code-quality watch items

These are small review notes worth preserving for future implementation lanes:

1. `SnapshotBundle::render_json()` hand-rolls JSON output to stay dependency-free. That is acceptable now, but future schema growth should be checked carefully so operator/report consumers do not drift from the documented shape.
2. `RuntimeSession` currently owns orchestration, metrics, latency, and latest-snapshot assembly in one struct. That keeps the scaffold simple today, but future file/report wiring should avoid turning it into a catch-all integration object.
3. The replay transport boundary is intentionally synchronous. If async live transports are added later, preserve the current fail-closed ordering instead of bypassing the orchestrator with side-channel state.

## Evidence reviewed

Verified locally with:

```bash
cd rust-copytrader
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Recommendation for integration

Treat this crate as the contract baseline for the Rust copy-trader lane:

- keep live mode fail-closed until the external leader-activity capability is actually verified
- keep latency rejection ahead of submit when remaining budget is exhausted
- keep verification mismatch/timeout distinct from submit failure
- preserve replay parity when real transports are introduced

If future workers add real transport I/O, they should do so by extending these boundaries, not by bypassing the orchestrator/session contracts already in place.
