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
- `src/adapters/submit_pipeline.rs` now composes auth material validation, order signing, L2 header provisioning, authenticated request building, and command-runner execution into one end-to-end submit pipeline contract without caller-threaded raw header signature/timestamp fields.
- `src/config.rs` now adds explicit signing/submit adapter selections plus repo-local helper-oriented command-spec wiring so replay scaffolds and command-backed live execution surfaces can be selected without changing the fixed hot path.

### Lane 2: snapshots + telemetry in the runtime session
- `src/app.rs` provides `RuntimeSession` plus `RuntimeSessionRecorder`, tying bootstrap state, orchestrator results, runtime metrics, latency accounting, snapshot persistence, rotating local logs, and operator report generation together.
- `src/app.rs` also exposes a config-driven runtime bootstrap/session path that only treats live execution as ready when command-backed signing and HTTP submit adapters are explicitly selected and wired, while surfacing the repo-local L2 helper bridge alongside the order-sign helper contract.
- `src/main.rs` now exposes a read-only bootstrap report entrypoint so operators can point the crate at a repo root, confirm the repo-local helper contract that was loaded, and verify that live mode still stays blocked until the rest of the live gates turn green.
- `src/pipeline/trace_context.rs` records ordered stage timestamps so latency reporting stays aligned with the mandated execution sequence.
- `src/persistence/snapshots.rs` defines a stable local JSON snapshot shape plus retained session snapshot archives for operator/runtime evidence.
- `src/persistence/jsonl.rs` rotates append-only activity/order/verification logs under the local session root without introducing a database.
- `src/telemetry/metrics.rs`, `src/telemetry/latency.rs`, and `src/telemetry/report.rs` accumulate submit/reject/timeout counters, per-stage timing deltas, and operator-facing JSON/text report surfaces.

### Lane 3: transport boundaries and config-driven live execution selection
- `src/adapters/activity.rs` keeps `live_listen`, `shadow_poll`, and `replay` mode selection explicit and fail-closed.
- `src/adapters/transport.rs` now resolves replay/shadow/live transport skeletons from a `TransportBoundaryConfig`, rejects mixed boundary modes fail-closed, and keeps the live-mode gate plus replay parity intact across activity, positions, market quote, and verification frames.
- `src/adapters/positions.rs`, `src/adapters/market_ws.rs`, and `src/adapters/verification.rs` define the current boundary contracts for positions reconciliation, market quote validation, and post-submit verification correlation.
- `ExecutionAdapterConfig` keeps replay defaults fail-closed for live mode while allowing an explicit `command signing + HTTP submit` selection, including repo-local helper args for `scripts/sign_order.py --json` plus the derived `scripts/sign_l2.py --json` bridge, to flow through `RuntimeBootstrap` / `RuntimeSession` when operators are ready to wire real auth material around the scaffold.
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

### 3b. 当前 live submit / signing 选择的含义
- **默认配置仍然不会解锁 live submit**：`ExecutionAdapterConfig::default()` 仍然选择 replay scaffolds（`replay_stub` signing + `replay` submit），因此即使其它 live gate 都为 green，runtime 仍会因 `execution_surface_not_ready` 而 fail-closed。
- **显式 live 选择必须成对出现**：当前 runtime/session 只把 `command` signing + `http_command` submit 视为 live-ready 组合；单独切换其中一个不会偷偷放开 live mode，而且 runtime 只会暴露这组完整的 command wiring。
- **这只是接线位，不是 live 解锁声明**：这些 config surface 的作用是把 lane 1/2 的 contract 接到 runtime bootstrap 上，方便后续真实 auth material / command runner 接入；它们本身不意味着已经可以安全实盘。

### 3c. 当前 Rust 侧 env/root 接线方式
- **root 读取顺序与 brownfield Python 保持一致**：`AuthMaterial::from_root`、`ExecutionAdapterConfig::from_root`、`RuntimeBootstrap::from_root` / `RuntimeSession::from_root` 都会先读进程环境，再用 `<root>/.env`、`<root>/.env.local` 依次覆盖。
- **auth material 变量名直接兼容 brownfield**：
  - signer / address: `POLY_ADDRESS`（或 `SIGNER_ADDRESS`）
  - api creds: `CLOB_API_KEY`、`CLOB_SECRET`、`CLOB_PASS_PHRASE`
  - legacy aliases: `POLY_API_KEY`、`POLY_API_SECRET`、`POLY_PASSPHRASE`
  - private key: `PRIVATE_KEY`（优先）或 `CLOB_PRIVATE_KEY`
  - proxy metadata: `SIGNATURE_TYPE`、`FUNDER_ADDRESS`（或 `FUNDER`）
- **submit command wiring 也从 root 读取并保持 fail-closed**：
  - signing command: `RUST_COPYTRADER_SIGNING_PROGRAM`
  - submit command: `RUST_COPYTRADER_SUBMIT_PROGRAM`
  - base URL: `CLOB_BASE_URL` 或 `CLOB_HOST`
  - optional time budgets: `RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS`、`RUST_COPYTRADER_SUBMIT_MAX_TIME_MS`
- **root 加载后的 command wiring 会自动落到 repo-local helper contract**：
  - order signing helper args: `scripts/sign_order.py --json`
  - L2 header helper args: `scripts/sign_l2.py --json`
  - 这些 helper args 只是 Rust scaffold 暴露给 brownfield/local command bridge 的 contract surface；它们不会单独解锁 live mode
- **builder surface 不会偷偷放开 live mode**：`HttpSubmitter::from_live_execution_wiring` / `SubmitPipeline::from_live_execution_wiring` 会拒绝空 base URL 或空 command program；runtime 侧即使已经从 root 读到 wiring，也仍然要等 `LiveModeGate` 其余 gate 全绿才会进入 `LiveListen`。

示例 root（仍然只是 wiring，不代表 live 已解锁）：

```bash
cat > .env.local <<'EOF'
POLY_ADDRESS=0x...
CLOB_API_KEY=...
CLOB_SECRET=...
CLOB_PASS_PHRASE=...
PRIVATE_KEY=...
SIGNATURE_TYPE=0
RUST_COPYTRADER_SIGNING_PROGRAM=python3
RUST_COPYTRADER_SUBMIT_PROGRAM=curl
CLOB_HOST=https://clob.polymarket.com
RUST_COPYTRADER_SUBMIT_CONNECT_TIMEOUT_MS=75
RUST_COPYTRADER_SUBMIT_MAX_TIME_MS=150
EOF
```

可以直接用新的 bootstrap report entrypoint 验证 Rust runtime 读到了哪些 helper wiring：

```bash
cd rust-copytrader
cargo run -- --root ..
```

当 root 中已经配置了 `RUST_COPYTRADER_SIGNING_PROGRAM` / `RUST_COPYTRADER_SUBMIT_PROGRAM` 等变量时，输出会明确展示：

```text
requested_mode=live_listen
decision=blocked:activity_source_unverified
live_mode_unlocked=false
signing_command=python3 scripts/sign_order.py --json
l2_header_helper=python3 scripts/sign_l2.py --json
submit_command=curl
```

这个 report 是只读的 bootstrap smoke check：它证明 repo-local helper contract 已经被 runtime/bootstrap 端正确加载，但它同样会明确显示 live mode 仍然是 blocked，直到 activity / budget / capability / positions 等 gate 全部变绿。

如果希望 Rust 侧不仅打印 wiring，还要**实际调用 repo-local helper scripts** 做一轮 smoke，可以直接运行：

```bash
cd rust-copytrader
cargo run -- --smoke-helper --root ..
```

这个模式会：
- 从 root 读取 helper-driven execution wiring
- 实际执行 `scripts/sign_order.py --json`
- 实际执行 `scripts/sign_l2.py --json`
- 生成最终的 `submit_preview_program` / `submit_preview_args`
- 输出 `order_signature` / `order_salt` / `l2_signature` / `l2_timestamp`
- 同时保留 `decision=blocked:...`，不会把 smoke mode 当成 live unlock

如果想从 repo root 直接把 helper-driven command path 跑一遍，而不只是看 bootstrap report，也可以执行：

```bash
bash scripts/run_rust_helper_smoke.sh
```

这个 smoke 脚本会：
- 调 `scripts/sign_order.py --json`
- 调 `scripts/sign_l2.py --json`
- 再调 `cargo run -- --smoke-helper --root ..`

它仍然是 **fail-closed** 的：
- 缺少 `CLOB_*` / `PRIVATE_KEY` / `POLY_ADDRESS` 等关键环境变量会直接退出非零
- helper 脚本失败会直接退出非零
- 即使 helper wiring 已经加载成功，输出仍会明确显示 live mode 默认 blocked

如果你想把当前可用的 operator-facing smoke 路径一次性看完，也可以直接运行：

```bash
cd rust-copytrader
cargo run -- --operator-demo --root ..
```

这个模式会串起来：
- helper smoke
- replay-backed runtime smoke
- 最终 submit command preview
- public discovery 命令提示

它依然是只读/本地验证，不会替你触发真实网络 submit。
另外它现在还会把完整 demo 输出落到：
- `.omx/operator-demo/operator-demo-*.txt`
- `.omx/operator-demo/latest.txt`
并额外给出：
- `replay_submit_elapsed_ms=60`
- `replay_verified_elapsed_ms=82`
- `submit_hard_budget_ms=200`
- `selected_leader_wallet=...`
- `selected_leader_source=...`
- `runtime_subject_wallet=...`
- `runtime_subject_source=...`
- `leaderboard_preview_url=...`
- `leaderboard_preview_curl=...`
- `activity_preview_url=...`
- `activity_preview_curl=...`
- `leaderboard_capture_hint=... --output ../.omx/discovery/leaderboard-overall-day-pnl.json`
- `activity_capture_hint=... --output ../.omx/discovery/activity-<wallet>-trade.json`
- `leader_selection_hint=... --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env`
- `leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a`

如果你已经设置了 `COPYTRADER_DISCOVERY_WALLET`，operator demo 会优先用它；否则会回退到 `POLY_ADDRESS` / `SIGNER_ADDRESS`。

如果你更喜欢从 repo root 直接跑这条 operator flow，也可以执行：

```bash
bash scripts/run_rust_operator_demo.sh
```

## Public discovery commands you can run today

如果你现在想先看 **top 盈利者名单** 或者直接拉某个 trader 的 **公开 activity**，现在已经有不需要私钥的 Rust CLI 命令：

```bash
cd rust-copytrader
cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20
cargo run --bin fetch_user_activity -- --user 0x56687bf447db6ffa42ffe2204a05edaa20f55839 --type TRADE --limit 20
```

如果你想把 discovery 原始响应直接落到 `.omx/` 下面做后续人工筛选，也可以：

```bash
cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --output ../.omx/discovery/leaderboard-overall-day-pnl.json
cargo run --bin fetch_user_activity -- --user 0x56687bf447db6ffa42ffe2204a05edaa20f55839 --type TRADE --limit 20 --output ../.omx/discovery/activity-0x56687bf447db6ffa42ffe2204a05edaa20f55839-trade.json
```

`--output` 会自动创建父目录，所以 `.omx/discovery/` 不需要你手动先 `mkdir -p`。

如果你想把 leaderboard 产物直接转成后续 operator 会读取的 leader 选择 env，也可以：

```bash
cargo run --bin select_copy_leader -- --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

后续再跑 `--operator-demo` 时，会优先读取 `.omx/discovery/selected-leader.env` 里的 `COPYTRADER_DISCOVERY_WALLET`。

如果你不是从 leaderboard 选，而是想直接把某个 trader 的 activity 产物转成 leader env，也可以：

```bash
cargo run --bin select_copy_leader -- --activity ../.omx/discovery/activity-0x56687bf447db6ffa42ffe2204a05edaa20f55839-trade.json --output ../.omx/discovery/selected-leader.env
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

如果你想把“抓 leaderboard -> 选 leader -> 抓这个 leader 的 activity -> 写 selected-leader.env”压成一个更真实的一步，也可以直接跑：

```bash
cargo run --bin discover_copy_leader -- --discovery-dir ../.omx/discovery
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

这个命令会：
- 抓 leaderboard 到 `.omx/discovery/leaderboard-*.json`
- 选出一个 leader wallet
- 抓这个 wallet 的 activity 到 `.omx/discovery/activity-*.json`
- 写 `.omx/discovery/selected-leader.env`

如果你想把 discovery + 选 leader + operator demo 一把串起来，也可以：

```bash
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery
```

如果网络环境有问题，或者你想先看它到底会打什么请求：

```bash
cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --print-url
cargo run --bin fetch_user_activity -- --user 0x56687bf447db6ffa42ffe2204a05edaa20f55839 --type TRADE --limit 20 --print-curl
```

它们调用的是官方 public Data API：
- leaderboard: `https://data-api.polymarket.com/v1/leaderboard`
- activity: `https://data-api.polymarket.com/activity`

这两个 Rust 命令的定位是：
- **discovery / watchlist / manual inspection**
- 不是 live 跟单解锁
- 不会改变当前 fail-closed 的 hot-path 决策

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
- `tests/submit_pipeline.rs` — end-to-end composition from auth material + unsigned order + L2 header provider to executed submit command output
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

1. real cryptographic signing material, command execution hardening, and live network execution backed by production auth/runtime inputs instead of scaffold-only adapter selection (the runtime can now select command-signing + HTTP-submit surfaces, but that does **not** claim live unlock)
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
