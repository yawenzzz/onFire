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
- `src/persistence/snapshots.rs` defines a stable JSON shape for local runtime evidence.
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

### Remaining gaps

These are remaining breadth gaps, not correctness defects in the implemented scaffold:

1. The preview/submit boundary is still an in-process contract; no concrete authenticated venue/router client is wired yet.
2. `RuntimeSession` currently retains/render snapshots in memory, but does not yet own session-level file persistence/rotation.
3. External metrics export and operator-facing reporting remain outside the crate.
4. Adapter boundaries are currently concrete module contracts rather than async trait-object integrations; that is acceptable for the scaffold, but future live wiring should preserve replay parity and fail-closed behavior.

## Evidence reviewed

Verified locally with:

```bash
cd rust-copytrader
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
