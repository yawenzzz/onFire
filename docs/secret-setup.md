# Secret Setup

Use local environment variables only.

## Generate fresh PM auth in your current shell
For Polymarket WebSocket auth, keep the long-lived developer credentials locally and derive fresh `PM_*` values on demand:

```bash
export POLYMARKET_KEY_ID=...
export POLYMARKET_SECRET_KEY=...

source scripts/generate_pm_auth.sh
```

This script:
- reads `POLYMARKET_KEY_ID`
- reads `POLYMARKET_SECRET_KEY`
- derives fresh `PM_ACCESS_KEY`, `PM_TIMESTAMP`, and `PM_SIGNATURE`
- defaults to signing `GET /v1/ws/markets`

Optional path override:

```bash
PM_PATH=/v1/ws/private source scripts/generate_pm_auth.sh
```

Do not store generated `PM_TIMESTAMP` or `PM_SIGNATURE` as long-lived values; they are meant to be used immediately.

## Generate CLOB credentials into `.env.local`
If you already have the wallet inputs locally, you can generate the `CLOB_API_KEY`, `CLOB_SECRET`, and `CLOB_PASS_PHRASE` triplet and write it into `.env.local` with one command:

```bash
export PRIVATE_KEY=...
# optional:
# export SIGNATURE_TYPE=0
# export FUNDER=0x...

bash scripts/generate_clob_env.sh
```

The script:
- calls `scripts/derive_clob_creds_python_template.py`
- creates `.env.local` from `.env.local.example` if needed
- writes the generated `CLOB_*` values into `.env.local`
- verifies the file with `scripts/check_secrets.sh`

It does not write `PRIVATE_KEY` into `.env.local`.

## Repo-local Rust helper scripts
The Rust copytrader live wiring expects repo-local command helpers rather than inline signing logic:

- order signing helper: `scripts/sign_order.py --json`
- L2 header helper: `scripts/sign_l2.py --json`
- submit helper wrapper: `scripts/submit_helper.py --json --curl-bin curl`

Both signing helpers read JSON from stdin, bridge the local `py-clob-client` install plus your existing env vars, and fail closed when the SDK or required secrets are missing. They provide the repo-local contract used by the Rust command adapters; they do **not** unlock live mode on their own.

## Run the repo-local Rust helper smoke path
Once `.env.local` (or shell env) contains the `CLOB_*`, `PRIVATE_KEY`, and optional proxy/funder fields, you can run the helper-driven smoke path directly from the repo root:

```bash
bash scripts/run_rust_helper_smoke.sh
```

This smoke script:
- signs a sample order via `scripts/sign_order.py --json`
- signs a sample L2 header payload via `scripts/sign_l2.py --json`
- runs the Rust helper smoke mode (`cargo run -- --smoke-helper --root ..`) to prove the helper-driven command path is loaded, callable, and able to preview the final submit command

It is still **fail-closed**:
- missing env -> exits non-zero
- helper import/setup failure -> exits non-zero
- live mode remains blocked unless the separate `LiveModeGate` conditions are all green

## Run the repo-local Rust runtime smoke path
If you want a replay-backed runtime/operator artifact pass on top of the helper smoke, run:

```bash
bash scripts/run_rust_runtime_smoke.sh
```

This script calls:

```bash
cd rust-copytrader
cargo run -- --smoke-runtime --root ..
```

It will:
- execute the helper-driven command path
- run a replay-backed runtime session
- emit `session_outcome`, `runtime_mode`, and `last_submit_status`
- emit `runtime_subject_wallet=...` / `runtime_subject_source=...` so the replay artifact shows which selected leader it is simulating
- emit replay latency evidence like `replay_submit_elapsed_ms=60`, `replay_verified_elapsed_ms=82`, and `submit_hard_budget_ms=200`
- print the generated snapshot/report artifact paths

## Run the repo-local Rust operator demo
如果你想把当前这条 operator-facing 主路一次性走完，可以直接运行：

```bash
bash scripts/run_rust_operator_demo.sh
```

这个脚本会调用：

```bash
cd rust-copytrader
cargo run -- --operator-demo --root ..
```

它会把：
- helper smoke
- replay-backed runtime smoke
- 最终 submit preview
- Rust discovery 命令提示

串成一份更完整的 operator demo 输出，同时仍保持 live mode blocked by default。
它还会把完整 demo 文本落到：
- `.omx/operator-demo/operator-demo-*.txt`
- `.omx/operator-demo/latest.txt`
并补充 discovery preview：
- `selected_leader_wallet=...`
- `selected_leader_source=...`
- `leaderboard_preview_url=...`
- `leaderboard_preview_curl=...`
- `activity_preview_url=...`
- `activity_preview_curl=...`
- `leaderboard_capture_hint=... --output ../.omx/discovery/leaderboard-overall-day-pnl.json`
- `activity_capture_hint=... --output ../.omx/discovery/activity-<wallet>-trade.json`
- `leader_selection_hint=... --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env`
- `leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a`

`--output` 会自动创建父目录，所以 `.omx/discovery/` 不需要手工先建。

如果你想把 leaderboard discovery 产物明确转成后续 operator/read-only 跟单会消费的 leader env，可以直接运行：

```bash
cd rust-copytrader
cargo run --bin select_copy_leader -- --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

这个 env 文件会包含：
- `COPYTRADER_DISCOVERY_WALLET=...`
- `COPYTRADER_LEADER_WALLET=...`
- `COPYTRADER_SELECTED_FROM=...`

如果你已经先抓了某个 trader 的 activity，也可以直接从 activity JSON 生成相同的 leader env：

```bash
cd rust-copytrader
cargo run --bin select_copy_leader -- --activity ../.omx/discovery/activity-0x56687bf447db6ffa42ffe2204a05edaa20f55839-trade.json --output ../.omx/discovery/selected-leader.env
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

如果你想一步跑完 leaderboard discovery + 选 leader + 抓 activity + 写 env，也可以：

```bash
cd rust-copytrader
cargo run --bin discover_copy_leader -- --discovery-dir ../.omx/discovery
set -a && source ../.omx/discovery/selected-leader.env && set +a
```

它还会直接打印真实 summary 字段，例如：
- `selected_rank=...`
- `selected_pnl=...`
- `selected_username=...`
- `latest_activity_side=...`
- `latest_activity_slug=...`

如果你想把这条 discovery + 选 leader + operator demo 直接串成一把，也可以：

```bash
cd rust-copytrader
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery
```

如果你想开始真实轮询当前选中的 leader activity，也可以：

```bash
cd rust-copytrader
cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --poll-count 1
```

它会把轮询结果落到：
- `.omx/live-activity/<wallet>/latest-activity.json`
- `.omx/live-activity/<wallet>/activity-events.jsonl`

如果你想把这条真实 activity 再推进到受控 runtime 一次处理，也可以：

```bash
cd rust-copytrader
cargo run --bin run_copytrader_guarded_cycle -- --root ..
```

它会读取：
- `.omx/discovery/selected-leader.env`
- `.omx/live-activity/<wallet>/latest-activity.json`

然后跑一轮 guarded replay runtime，并把 artifact 落到 `.omx/guarded-cycle/`。

如果默认 `data-api.polymarket.com` / `activity` host 在你当前环境里连不通，也可以先覆盖：

```bash
export POLYMARKET_LEADERBOARD_BASE_URL=https://your-proxy.example/leaderboard
export POLYMARKET_ACTIVITY_BASE_URL=https://your-proxy.example/activity
cd rust-copytrader
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery
```

如果 host 不变，只是需要显式代理，也可以：

```bash
export POLYMARKET_CURL_PROXY=http://127.0.0.1:7897
cd rust-copytrader
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery
```

如果代理偶发超时或者 `SSL_ERROR_SYSCALL`，也可以直接给 discovery / watcher 提高重试次数：

```bash
export POLYMARKET_CURL_PROXY=http://127.0.0.1:7897
cd rust-copytrader
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery --retry-count 3 --retry-delay-ms 1000
```

它仍然是 **fail-closed** 的：
- helper / runtime smoke 都只做本地验证
- 仍然不会解锁 live mode
- 也不会替你执行真实网络 submit

## Run the CLOB account-only view
The account-only monitor path uses CLOB Level 2 auth. In practice that means you need:

- `CLOB_API_KEY`
- `CLOB_SECRET`
- `CLOB_PASS_PHRASE`
- `PRIVATE_KEY` or `CLOB_PRIVATE_KEY`

And per the official CLOB auth docs, many Polymarket accounts also need:

- `SIGNATURE_TYPE`
- `FUNDER_ADDRESS`

Official values:
- `0` = EOA
- `1` = POLY_PROXY
- `2` = GNOSIS_SAFE

The docs say the Polymarket-displayed wallet/proxy address should be used as the funder address.

Without the private key, the monitor will stay fail-closed and show:
- `mode=account-ready`
- `reason=missing private key for clob level2 auth`

Example:

```bash
export PRIVATE_KEY=...
# or:
# export CLOB_PRIVATE_KEY=...
export SIGNATURE_TYPE=0
# or 1 / 2 depending on account type
export FUNDER_ADDRESS=0x...

PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.monitor_cli --root . --account-only
```

## Manual synchronous order draft
The current live trading path is **manual and synchronous**:

- configure one explicit order draft locally
- open the account-only TUI
- press `p`
- the app submits once and immediately refreshes the account snapshot

Required draft variables:

```bash
export ORDER_TOKEN_ID=...
export ORDER_SIDE=BUY
export ORDER_PRICE=0.42
export ORDER_SIZE=5
export ORDER_TYPE=GTC
```

Supported order types currently passed through to the CLOB client:
- `GTC`
- `FOK`
- `FAK`
- `GTD`

Example full command:

```bash
set -a
source .env.local
set +a

PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.monitor_cli --root . --account-only
```

Then inside the TUI:
- `Tab` switches focus
- `j/k` and `PgUp/PgDn` scroll the focused section
- press `p` to submit the configured draft once

If the draft is missing or malformed, the monitor stays fail-closed and shows the error instead of crashing.

## Shell exports
```bash
export PRIVATE_KEY=...
export CLOB_PRIVATE_KEY=...
export SIGNATURE_TYPE=...
export FUNDER_ADDRESS=...
export ORDER_TOKEN_ID=...
export ORDER_SIDE=BUY
export ORDER_PRICE=...
export ORDER_SIZE=...
export ORDER_TYPE=GTC
export POLYMARKET_KEY_ID=...
export POLYMARKET_SECRET_KEY=...
export PM_ACCESS_KEY=...
export PM_SIGNATURE=...
export PM_TIMESTAMP=...
export CLOB_API_KEY=...
export CLOB_SECRET=...
export CLOB_PASS_PHRASE=...
```

## Local file
Create `.env.local`, then load it:
```bash
set -a
source .env.local
set +a
```

do not commit secrets to git.
