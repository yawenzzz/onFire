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

## 当前跟单策略（Current copy-trading posture）

这份 README 当前描述的不是“收益优化策略”，而是**真值优先、延迟受限、默认 fail-closed 的跟单执行策略**。它的目标是：在不破坏固定热路径的前提下，只对已经被证实的 leader 仓位变化做出可验证的跟随动作。

### 1. 当前只接受哪种“可跟单”信号
- **先看到 leader activity，再继续后续阶段**：热路径固定为 `activity -> positions -> market websocket -> submit -> verification`。
- **activity 只是线索，不是最终真值**：系统不会因为看到 leader trade event 就直接下单，必须继续经过 `/positions` 对 leader 净仓位变化做确认。
- **没有净仓位变化就不跟**：如果 reconciliation 后发现 leader 仓位没有真实变化，系统会 reject，而不是“猜测性”提交。
- **quote 必须新鲜**：只有在 market quote freshness、price shape、market-open 等条件都满足时，才会进入 submit-ready intent。

### 2. 当前策略会做什么决策
- **本质上是 confirmed position delta follower**：
  - 观察到 leader 事件；
  - 用 `/positions` 验证该 leader 在对应市场/asset 上确实产生净变化；
  - 结合最新 quote 构造一个满足预算和预检规则的跟单意图；
  - 通过 preview / submit / verification 完成一次受控执行。
- **当前不做自动策略优化**：
  - 不做 leader 打分或自动 leader 轮换；
  - 不做跨市场组合优化；
  - 不做动态仓位管理优化；
  - 不做为了命中率而绕过真值路径的推断式提交。

### 3. 当前策略的硬边界
- **200ms submit ceiling**：从 leader event ingress 到 submit acknowledgment 的热路径超过 200ms 时，系统应该 reject，而不是继续下单。
- **默认 fail-closed**：任一关键前提不成立都会阻断执行，例如：
  - live activity source 未验证；
  - `/positions` 超时或数据陈旧；
  - quote 陈旧或 spread/price 结构非法；
  - auth/runtime 未 ready；
  - preview 失败；
  - mixed transport mode 破坏 replay/live parity。
- **submit 和 verification 明确分离**：
  - submit failure 是该次尝试的终态；
  - 只有 accepted submit 才进入 verification-pending；
  - verification mismatch / timeout 会进入显式 incident 路径。

### 4. 当前运行模式的含义
- **replay**
  - 用 fixture 驱动完整热路径；
  - 是当前验证固定路径、预算和状态机行为的主模式。
- **shadow_poll**
  - 允许保留影子模式的活动输入/轮询思路；
  - 但不会被当成已经满足 live 200ms 目标的真实证据。
- **live_listen**
  - 仍然被 gate 住；
  - 只有当第三方 leader activity source 被证实存在、官方/实验能力成立、并满足预算要求后才能解锁。

### 5. 用一句话概括当前策略
> **先证实 leader 的真实净仓位变化，再在新鲜 quote 和严格预算下尝试跟单；任何一步不能被证实，就拒绝执行。**

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
