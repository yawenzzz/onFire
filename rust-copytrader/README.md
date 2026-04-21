# rust-copytrader 用户指南（说人话版）

这不是一个“概念验证玩具”，也不是一个已经默认会替你真下单的黑盒。

它现在是一条 **Rust-first 的跟单主路**，已经打通到：

1. **真实 discovery**（抓 Polymarket leaderboard / activity）
2. **真实 leader selection**（选出要跟的 leader）
3. **真实 activity watcher**（持续看这个 leader 的公开 activity）
4. **guarded runtime cycle**（把这条 activity 喂进受控执行链）
5. **live submit gate**（默认只做到 `preview_only`，不会偷偷真下单）

一句话：

> **现在已经可以真实抓榜、真实选 leader、真实盯 activity、真实生成 live submit preview。**  
> **默认还是安全的，不会自动给你真下单。**

---

## 1. 现在到底能做什么，不能做什么

### 已经能做的
- 从 Polymarket 公开数据里抓 top trader
- 自动选一个 leader，写入 `.omx/discovery/selected-leader.env`
- 继续抓这个 leader 的 activity
- 把 activity 落到 `.omx/live-activity/...`
- 跑一轮 guarded runtime
- 生成 **真实 submit preview**（签名、L2 header、payload 都会真生成）
- 把整条 operator flow 落成报告

### 还没默认替你做的
- **真实 live submit**（真钱 side effect）
- **真实 verification 回执后的实盘 latency 证明**

所以当前状态很明确：

- **preview 路已经通了**
- **真下单按钮还需要你显式按**

---

## 2. 先记住这四个安全级别

### Level 0：只看，不联网交易
只做 discovery / report / replay smoke。

### Level 1：真实公开数据 + 不下单
会打 Polymarket data API，但不会生成 submit 请求。

### Level 2：真实公开数据 + 真实 submit preview
会真的生成：
- signed order
- L2 headers
- submit payload

但是结果是：
- `live_submit_status=preview_only`

### Level 3：真实 live submit
只有你显式加：
- `--allow-live-submit`

才会真的尝试往 CLOB 发单。

**默认不做。**

---

## 3. 目录结构：你会看到哪些产物

所有运行证据基本都在 repo root 的 `.omx/` 下：

- `.omx/discovery/`
  - leaderboard 原始抓取
  - activity 原始抓取
  - `selected-leader.env`
- `.omx/live-activity/<wallet>/`
  - `latest-activity.json`
  - `activity-events.jsonl`
  - `seen-tx.txt`
- `.omx/live-submit/`
  - live submit gate 报告

如果你只想知道“这次到底跑成什么样”，优先看：

- `.omx/live-submit/live-submit-*.txt`

账户监控（一次性快照 / 持续轮询 / user-channel websocket）单独文档见：

- `ACCOUNT_MONITOR.md`
- `COPYTRADE_LATENCY.md`

---

## 4. 运行前准备

### 4.1 Rust
你至少需要：

- Rust / Cargo

---

### 4.2 代理
如果你当前网络直连 Polymarket data API 不稳定，直接开代理：

```bash
export https_proxy=http://127.0.0.1:7897
export http_proxy=http://127.0.0.1:7897
export all_proxy=socks5://127.0.0.1:7897
```

但是要注意：

- **discovery / watcher 这种联网命令**：推荐显式加 `--proxy http://127.0.0.1:7897`
- **signing helper / L2 helper**：现在已经做了代理隔离，不会再被 SOCKS 配置误伤

也就是说：

> 网络命令走显式 `--proxy`，本地签名命令不要靠全局代理碰运气。

---

### 4.3 本地 auth / secret
如果你要走到 preview gate，至少需要本地 auth material。

常见变量：

```bash
POLY_ADDRESS=0x...
CLOB_API_KEY=...
CLOB_SECRET=...
CLOB_PASS_PHRASE=...
PRIVATE_KEY=...
SIGNATURE_TYPE=0
FUNDER_ADDRESS=...
CLOB_HOST=https://clob.polymarket.com
COPYTRADER_MAX_TOTAL_EXPOSURE_USDC=100
COPYTRADER_MAX_ORDER_USDC=10
COPYTRADER_ACCOUNT_SNAPSHOT_PATH=runtime-verify-account/dashboard.json
COPYTRADER_ACCOUNT_SNAPSHOT_MAX_AGE_SECS=300
COPYTRADER_ACTIVITY_MAX_AGE_SECS=60
```

放在 repo root：

- `.env`
- 或 `.env.local`

当前 Rust 侧读取顺序是：
1. 进程环境
2. `<root>/.env`
3. `<root>/.env.local`

### 一个重要改动
现在 **preview gate 已经不再强依赖你手工填 `POLY_ADDRESS`**。
如果本地只有：
- 私钥
- CLOB creds

但没有 `POLY_ADDRESS`，系统会尝试：
- 通过 repo-local signing helper 反推出 signer address

所以你少填一个地址，不会再直接卡死在 preview lane。

---

## 5. 最推荐的使用顺序（先安全，再逐步逼近真单）

下面按“最稳妥”的顺序来。

---

## 6. 第一步：看当前 bootstrap / helper wiring 有没有读对

### 6.1 只看 bootstrap report

```bash
cd ~/onFire/rust-copytrader
cargo run -- --root ..
```

你会看到类似：

```text
requested_mode=live_listen
decision=blocked:activity_source_unverified
live_mode_unlocked=false
signing_command=rust_sdk
submit_command=curl
```

这说明：
- wiring 读到了
- 但 live mode 还没放开
- 这是正常的

---

## 7. 第二步：真实轮询 leader activity

如果你已经知道要跟的 wallet，直接跳过 discovery，开始实时轮询：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --user <wallet> --poll-count 1
```

它会把结果落到：
- `.omx/live-activity/<wallet>/latest-activity.json`
- `.omx/live-activity/<wallet>/activity-events.jsonl`
- `.omx/live-activity/<wallet>/seen-tx.txt`

这一步的意义是：

> 你已经不是“拿静态榜单”了，而是在盯一个真实 leader 的公开 activity。

---

## 9. 第四步：跑到 live submit gate（默认安全）

### 10.1 单独跑 preview gate

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_live_submit_gate -- \
  --root .. \
  --max-total-exposure-usdc 100 \
  --max-order-usdc 10 \
  --account-snapshot runtime-verify-account/dashboard.json
```

如果一切正常，你会看到：

```text
live_gate_status=unlocked
live_submit_status=preview_only
```

这就是现在最关键的“最后一米”状态。

它表示：
- 真实 activity 已经进入 live gate
- 真正的 signed order 已经生成
- 真正的 L2 header 已经生成
- 真正的 submit request preview 已经生成
- 但默认还是 **只预览，不发真单**

### 10.2 报告在哪里
这一步会落：

- `.omx/live-submit/live-submit-*.txt`

里面能看到：
- `activity_tx=...`
- `activity_side=...`
- `activity_asset=...`
- `activity_age_secs=...`
- `preview_readiness=...`
- `live_submit_readiness=...`
- `risk_gate_status=...`
- `unsigned_maker_amount=...`
- `unsigned_taker_amount=...`
- `preview_program=...`
- `preview_args=...`

### 10.3 新的安全语义
- `preview_args` 现在会**自动脱敏**，不再把 `POLY_API_KEY` / `POLY_SIGNATURE` 之类的敏感头原样写进报告
- 如果 `selected-leader.env` 里是占位 wallet（比如 `0xleader`），live path 会直接 fail-closed
- 如果你给了 `--max-total-exposure-usdc` / `--max-order-usdc`，gate 会尝试从 account snapshot 推导当前暴露并给出：
  - `risk_current_total_exposure_usdc=...`
  - `risk_projected_total_exposure_usdc=...`
  - `risk_gate_status=ready|blocked:*`
- 判断能不能**真下单**时，优先看：
  - `live_submit_readiness`
  - 再看 `risk_gate_status`
- `preview_readiness=ready` 只表示“可以生成 preview”，**不等于**“可以 live submit”
- `risk_gate_status=blocked:*` 在 preview 下只是提示；在 `--allow-live-submit` 下会直接阻止真下单

---

## 10. 真下单怎么做（先看再决定）

先说结论：

> **代码已经到真下单门口，但默认不会替你按这个按钮。**

如果你真的要尝试 live submit，需要显式加：

```bash
--allow-live-submit
```

例如：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_live_submit_gate -- \
  --root .. \
  --max-total-exposure-usdc 100 \
  --max-order-usdc 10 \
  --account-snapshot runtime-verify-account/dashboard.json \
  --allow-live-submit
```

### 强提醒
这一步可能会有：
- 真正的资金 side effect
- 真正的挂单 / 吃单 / 拒单
- 真正的交易后果

所以：
- **不确认资金账户状态，不要跑**
- **不确认 market / token / signer / proxy signature 配置，不要跑**
- **不确认你就是要上真单，不要跑**

---

## 14. 当前延时怎么理解

当前仓库里最可靠、反复验证过的延时证据是：

- `submit ack = 60ms`
- `verified = 82ms`
- `hard budget = 200ms`
- `headroom = 140ms`

但是一定要按真实语义理解：

> 这是 **replay / guarded runtime 证据**，  
> **不是 live 实盘 submit latency 证据**。

别把它理解成“已经实盘 60ms 下单了”。

现在准确说法是：
- **preview / guarded 执行链证明了这条热路径是快的**
- **但真实 live submit latency 还要等你真正执行 `--allow-live-submit` 后再看**

---

## 15. 最常用命令速查表

### 15.1 Bootstrap

```bash
cd ~/onFire/rust-copytrader
cargo run -- --root ..
```

### 15.2 Watch

```bash
cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --poll-count 1
```

### 15.4 Live preview / live submit

```bash
cargo run --bin run_copytrader_live_submit_gate -- --root .. --max-total-exposure-usdc 100 --max-order-usdc 10 --account-snapshot runtime-verify-account/dashboard.json
cargo run --bin run_copytrader_live_submit_gate -- --root .. --max-total-exposure-usdc 100 --max-order-usdc 10 --account-snapshot runtime-verify-account/dashboard.json --allow-live-submit
```


并且把报告落到：
- `.omx/position-targeting/position-targeting-*.txt`

## 15.4c min-max activity 跟单策略（新）

如果你想做一个**超简单、低延时、按 leader 最近 activity 金额归一化开仓**的策略，现在可以直接跑：

```bash
cd ~/onFire
bash scripts/run_rust_minmax_follow.sh --user <wallet>
```

它会做：

- 盯指定 wallet 的最新 `/activity`
- 取最近一批 `TRADE.usdcSize`
- 用 `min-max` 归一化成 `1..100`
- 把这个分数直接映射成 `1..100 USDC` 的开仓金额
- 再走现有 `run_copytrader_live_submit_gate`

默认是 preview / gate 模式，不会直接 live submit。

常用例子：

```bash
# 跑一次 preview
bash scripts/run_rust_minmax_follow.sh \
  --user 0xae7c98235d5dc797edfa3d3af2e0334238a4487e \
  --loop-count 1

# 改仓位范围，比如 5~50 USDC
bash scripts/run_rust_minmax_follow.sh \
  --user 0xae7c98235d5dc797edfa3d3af2e0334238a4487e \
  --min-open-usdc 5 \
  --max-open-usdc 50 \
  --loop-count 1

# 真正放开 live submit（前提是你的 live gate / 签名 / 下单配置都已经准备好）
bash scripts/run_rust_minmax_follow.sh \
  --user 0xae7c98235d5dc797edfa3d3af2e0334238a4487e \
  --max-total-exposure-usdc 100 \
  --max-order-usdc 10 \
  --account-snapshot runtime-verify-account/dashboard.json \
  --allow-live-submit
```

输出里你会直接看到：

- `latest_activity_usdc`
- `recent_usdc_min`
- `recent_usdc_max`
- `normalized_score`
- `normalized_open_usdc`
- `planned_open_usdc`
- `condition_key`
- `condition_outcome`
- `condition_same_outcome_sum_usdc`
- `condition_opposite_outcome_sum_usdc`
- `condition_decision`
- `live_submit_status`

落盘位置：

- `.omx/minmax-follow/<wallet>/`
- `.omx/minmax-follow/<wallet>/latest.txt`
- `.omx/minmax-follow/<wallet>/latest.json`

如果这轮真的走到了 `live_submit_status=submitted`，Rust 跟单循环还会在提交后再刷新一次 account snapshot，方便下一轮风控和后续对账。

另外现在每轮会明确写：

- `auto_submit_enabled=true|false`
- `submit_mode=live|preview`

所以如果你没看到真下单，先看这两个字段，不要只看 wrapper 启动命令。

如果你要**持续实时检测 + 自动下单**，直接跑：

```bash
bash scripts/run_rust_minmax_follow_live.sh \
  --user 0xae7c98235d5dc797edfa3d3af2e0334238a4487e
```

这个 live 脚本默认会带：

- `--min-open-usdc 0.1`
- `--max-open-usdc 10`
- `--max-total-exposure-usdc 100`
- `--max-order-usdc 10`
- `--account-snapshot runtime-verify-account/dashboard.json`
- Rust 跟单循环内每轮自动用 `run_rust_show_account_info.sh` 刷新 `runtime-verify-account/dashboard.json`
- 如果 watch 本轮返回 `poll_new_events=0`，会直接 `submit_status=skipped_no_new_activity`
- 进程失败时默认自动重启（最多 20 次，每次间隔 5 秒）
- **不会默认 `--allow-live-submit`**
- **不会默认 `--forever`**

默认每 `500ms` 检一次，你也可以改：

```bash
MIN_OPEN_USDC=5 \
MAX_OPEN_USDC=50 \
MAX_TOTAL_EXPOSURE_USDC=100 \
MAX_ORDER_USDC=10 \
LOOP_INTERVAL_MS=300 \
WATCH_LIMIT=80 \
bash scripts/run_rust_minmax_follow_live.sh --user <wallet>
```

现在这些 wrapper **默认不再内置代理**。  
如果你需要走代理，显式外部传参即可，例如：

```bash
bash scripts/run_rust_minmax_follow_live.sh --user <wallet> --proxy http://127.0.0.1:7897
```

或者：

```bash
POLYMARKET_CURL_PROXY=http://127.0.0.1:7897 \
bash scripts/run_rust_minmax_follow_live.sh --user <wallet>
```

如果你想关掉自动重启：

```bash
RESTART_ON_FAILURE=0 \
bash scripts/run_rust_minmax_follow_live.sh --user <wallet>
```

如果你想调重启策略：

```bash
RESTART_ON_FAILURE=1 \
MAX_RESTARTS=50 \
RESTART_DELAY_SECONDS=3 \
bash scripts/run_rust_minmax_follow_live.sh --user <wallet>
```

每轮报告现在还会带：

- `poll_transport_mode=proxy|direct_fallback|direct`
- `watch_has_new_activity=true|false`
- `account_snapshot_refresh_status=ok|failed|disabled:*`
- `submit_status=skipped_no_new_activity|skipped_account_snapshot_refresh_failed|skipped_condition_unconfirmed_small_entry|skipped_condition_hedge_candidate|skipped_duplicate_tx|skipped_live_gate_blocked|...`

其中 condition-aware 跟单规则现在是：

- `condition_unconfirmed_small_entry`
  - 同一事件下第一次看到的这笔太小，先不跟，避免把零碎试探单/对冲单当成新开仓
- `condition_hedge_candidate`
  - 同一事件下 opposite outcome 的历史资金明显更大，而当前这笔又很小，默认视为对冲/减仓信号，不新开反向跟单
- `condition_follow_confirmed`
  - 同一事件、同一 outcome 已经有足够确认（单笔够大、累计够大，或连续多笔），才允许按 `planned_open_usdc` 跟单

如果你要真的持续 live submit，需要显式打开：

```bash
FOLLOW_FOREVER=1 \
AUTO_SUBMIT=1 \
bash scripts/run_rust_minmax_follow_live.sh --user <wallet>
```

如果你不想每次手动配这两个开关，也可以直接用专门的 real-submit launcher：

```bash
bash scripts/run_rust_minmax_follow_live_submit.sh --user <wallet>
```

这个 launcher 现在默认会**循环复用**：
- `bash scripts/run_rust_follow_last_action_force_live_once.sh --user <wallet>`

也就是每轮都按“最新一笔动作 -> force live submit once”的已验证真路径执行。
同时它会默认带：
- `REQUIRE_NEW_ACTIVITY=1`

所以如果 watch 本轮返回 `poll_new_events=0`，continuous wrapper 会**安全跳过**，不会在首次启动时盲追一笔旧单。

默认会：
- `FOLLOW_FOREVER=1`
- `RESTART_ON_FAILURE=1`
- `LOOP_DELAY_SECONDS=1`

如果你想先做**单次真开单**而不是一直挂循环，也可以直接用：

```bash
bash scripts/run_rust_minmax_follow_live_submit_once.sh --user <wallet>
```

这个 one-shot launcher 会直接复用：
- `bash scripts/run_rust_follow_last_action_force_live_once.sh --user <wallet>`

并且同样带：
- `REQUIRE_NEW_ACTIVITY=1`

如果你想完全跳过 minmax/seen 这类“策略/保护层”，只拿某个 wallet 的**最新一笔动作**做 1 USD 真开单验证，可以直接用：

```bash
bash scripts/run_rust_follow_last_action_force_live_once.sh --user <wallet>
```

如果 force-live-once 要走代理，也改成显式传：

```bash
bash scripts/run_rust_follow_last_action_force_live_once.sh \
  --user <wallet> \
  --proxy http://127.0.0.1:7897
```

这个 force-live 验证脚本会：
- 先尝试 `watch_copy_leader_activity`
- watch 失败但本地已有 `latest-activity.json` 时，直接用缓存动作继续
- 用 `--override-usdc-size 1`
- 带 `--allow-live-submit`
- 带 `--force-live-submit`

它是**危险验证入口**，只适合回答“到底能不能真下单”这个问题。

如果你要**安全 smoke / dry-run**，保持默认就行，或者显式写：

```bash
FOLLOW_FOREVER=0 \
AUTO_SUBMIT=0 \
bash scripts/run_rust_minmax_follow_live.sh \
  --user <wallet> \
  --loop-count 1
```

这样仍然会走实时策略链路，但只会走到 preview，不会 live submit。

## 16. 怎么测试（开发 / 回归）

### 16.1 Rust 侧

```bash
cd ~/onFire/rust-copytrader
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### 16.2 重点二进制

```bash
cargo test --bin rust-copytrader
cargo test --bin run_copytrader_live_submit_gate
```

## 17. 常见报错怎么处理

### 17.1 `Failed to connect to data-api.polymarket.com`
原因：网络不通 / 代理没配对。

处理：
- 开代理环境变量
- 并给 discovery / watcher 显式加：

```bash
--proxy http://127.0.0.1:7897
```

---

### 17.2 `missing field POLY_ADDRESS`
旧问题。当前 preview lane 已经基本兜住了。

如果还出现：
- 看 `.env` / `.env.local` 里有没有私钥和 CLOB creds
- 再看 `auth_env_source` / `auth_signer_address` / `auth_effective_funder_address` 是否符合预期

---

### 17.3 `Using SOCKS proxy, but the 'socksio' package is not installed`
这通常是全局 `all_proxy=socks5://...` 影响到了本地 helper。

当前仓库已经尽量隔离这个问题。
如果你还碰到：
- discovery/watch 继续用显式 `--proxy`
- 不要指望 signing helper 靠 SOCKS 环境工作

---

### 17.5 `live_submit_status=preview_only`
这不是报错，这是**正常安全结果**。

它表示：
- 预览链路通了
- 但你没有显式要求真下单

---

## 18. 现在项目完成度怎么理解

如果你关心的是：

> **Rust-first copytrader 主路是否已基本成型？**

答案是：**是。**

如果你关心的是：

> **是否已经默认自动替我真下单？**

答案是：**没有，默认不会。**

如果你问：

> **现在离真下单还有多远？**

答案是：

> **不只是 `--allow-live-submit`。你还需要：**
> - 新鲜 activity
> - 新鲜 account snapshot
> - 风控上限（total exposure / max order）
> - 正确 signer / funder / creds

也就是说：
- discovery 已经真连了
- watcher 已经真连了
- guarded cycle 已经真吃到 activity 了
- submit preview 已经真生成了
- 但 live gate 现在不会再接受“手工把 activity / positions 说成没问题”这种绕过方式
- 剩下就是真正放开 live submit

---

## 19. 我建议你怎么实际使用

### 如果你准备冲真下单
先别急，先确认三件事：
1. auth / signer / funder 配置完全对
2. 你知道当前选中的 market / token / side 到底是什么
3. 你确认要承担真钱后果

然后再去考虑：

```bash
--allow-live-submit
```

---

## 20. 最后一句

这个 README 对应的是当前仓库的真实状态：

> **它已经不是“想法”，而是一条能跑到 real live preview 的 Rust 主路。**  
> **默认安全，不乱下单；真下单需要你自己显式解锁。**
