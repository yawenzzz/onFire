# Rust Copytrader Scaffold Review

_Date:_ 2026-04-15  
_Scope reviewed:_ `rust-copytrader/`

## Summary

The committed Rust scaffold is a good contract-first starting point for the PRD. It already captures the most important safety boundaries:

- live mode is explicitly blocked until the leader-activity source is verified
- stale positions and stale quotes are rejected instead of tolerated
- the 200ms submit ceiling is represented as a hard budget, not an aspiration
- submit failure and post-submit verification failure are modeled as different states
- file persistence remains append-only and local-file-only

## What is already implemented well

### 1. Live-mode safety is fail-closed
`src/config.rs`, `src/app.rs`, and `src/adapters/activity.rs` collectively prevent unsupported `live_listen` mode from activating without explicit capability checks. This matches the PRD requirement to keep live mode gated while official leader-activity listening remains unresolved.

### 2. Core hot-path contracts are test-backed
The existing test suite covers:
- blocked live mode without verified activity source
- stale `/positions` rejection
- no-net-change suppression after reconciliation
- stale quote rejection and spread validation
- hard latency budget scheduling checks
- separation of submit failure from verification-pending states

### 3. The scaffold stays small and reviewable
The crate currently avoids premature abstractions, extra dependencies, or storage expansion beyond JSONL append. That keeps the diff easy to reason about and aligned with the single-node/local-file-only constraints.

## Gaps that remain open

These are implementation gaps, not regressions in the current scaffold:

1. No full orchestrator yet for the end-to-end ordered path `activity -> positions -> market websocket -> submit -> verification`.
2. No preview/order adapter wiring yet, so remaining-budget checks before submit are not enforced end-to-end.
3. No verification adapter yet, so verified/mismatch/timeout transitions are modeled but not fed by a concrete transport.
4. No deterministic replay harness yet, even though the design already assumes replay/shadow modes.
5. `TraceContext` currently records only a subset of the eventual stage timeline; later lanes should extend it carefully once the orchestrator exists.

## Recommendation for integration

When cherry-picking worker lanes into the leader branch, keep this scaffold positioned as a contract baseline. Add new runtime code by extending these existing contracts instead of bypassing them. In particular:

- do not relax the live-mode gate to “best effort” behavior
- do not allow submit attempts once the remaining budget is exhausted
- do not fold verification mismatch/timeout into generic submit errors
- do not replace append-only local durability with broader storage scope without an explicit scope change

## Suggested verification baseline for future lanes

Every lane that extends `rust-copytrader/` should continue to run at least:

```bash
cd rust-copytrader
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

If a later lane adds the real orchestrator or adapter I/O, add replay-based end-to-end tests before claiming the fixed path is implemented.
