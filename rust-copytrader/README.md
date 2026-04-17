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
- `.omx/guarded-cycle/`
  - guarded runtime session 证据
- `.omx/live-submit/`
  - live submit gate 报告
- `.omx/operator-demo/`
  - 一键 operator flow 的整合报告
- `.omx/auto-guarded/`
  - 自动循环 lane 的整合报告

如果你只想知道“这次到底跑成什么样”，优先看：

- `.omx/operator-demo/discover-and-demo-*.txt`
- `.omx/auto-guarded/auto-guarded-*.txt`
- `.omx/live-submit/live-submit-*.txt`

---

## 4. 运行前准备

### 4.1 Rust / Python
你至少需要：

- Rust / Cargo
- 一个可用 Python 3
- 本地可用的：
  - `py_clob_client`
  - `py_order_utils`

这个仓库当前环境默认优先用：
- `$HOME/anaconda3/bin/python3`

如果你想强制指定 Python：

```bash
export PYTHON_BIN=$HOME/anaconda3/bin/python3
```

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
signing_command=python3 scripts/sign_order.py --json
l2_header_helper=python3 scripts/sign_l2.py --json
submit_command=python3 scripts/submit_helper.py --json --curl-bin curl
```

这说明：
- wiring 读到了
- 但 live mode 还没放开
- 这是正常的

---

### 6.2 真正跑 helper smoke

```bash
cd ~/onFire/rust-copytrader
cargo run -- --smoke-helper --root ..
```

如果成功，你会看到：
- `helper_smoke=ok`
- `order_signature=...`
- `l2_signature=...`
- `submit_preview_program=...`

也就是说：
- 本地签名 helper 能跑
- L2 helper 能跑
- submit preview 能构造出来

你也可以直接用 root 脚本：

```bash
bash scripts/run_rust_helper_smoke.sh
```

---

## 7. 第二步：真实抓榜 + 真实选 leader

### 7.1 一把做完 discovery -> smart money 筛选 -> 选 leader

```bash
cd ~/onFire/rust-copytrader
cargo run --bin discover_copy_leader -- \
  --discovery-dir ../.omx/discovery \
  --proxy http://127.0.0.1:7897 \
  --category SPECIALIST \
  --connect-timeout-ms 8000 \
  --max-time-ms 20000
```

成功后你会得到：
- `.omx/discovery/leaderboard-*.json`
- `.omx/discovery/activity-*.json`
- `.omx/discovery/positions-*.json`
- `.omx/discovery/value-*.json`
- `.omx/discovery/traded-*.json`
- `.omx/discovery/wallet-filter-v1-report.txt`
- `.omx/discovery/selected-leader.env`

这里要注意一件事：

> `discover_copy_leader` 现在已经不是“按原始榜单 rank 直接拿第 1 名”。  
> 它会严格按 `rust-copytrader/wallet_filter_v1.md` 去做 smart money 钱包筛选：
> - 同类别 `WEEK + MONTH` 交集做候选池
> - `ALL + PNL` 只加分
> - `VOL` 榜只当红旗
> - 再叠 `/activity`、`/positions`、`/value`、`/traded`、市场元数据做硬过滤 + 打分

stdout 里会有类似：

```text
selected_wallet=0x...
selected_category=SPORTS
selected_score=87
core_pool_count=3
core_pool_wallets=0xaaa:95,0xbbb:88,0xccc:83
active_pool_count=2
active_pool_wallets=0xaaa:95,0xbbb:88
selected_rank=12
selected_week_rank=8
selected_month_rank=12
selected_all_rank=41
selected_pnl=...
selected_username=...
filter_report_path=../.omx/discovery/wallet-filter-v1-report.txt
latest_activity_side=BUY
latest_activity_slug=...
latest_activity_tx=0x...
```

如果你想看它为什么选这个 wallet，不要只看 `selected_wallet`，直接看：

```bash
cat ../.omx/discovery/wallet-filter-v1-report.txt
```

里面会直接告诉你每个候选钱包的：
- score
- maker rebate 情况
- flip60
- median hold
- tail24 / tail72
- copyable ratio
- neg risk share
- category purity
- unique markets / traded markets
- 以及被踢掉的原因

如果这个类别里真的有通过硬过滤的钱包，现在还会额外给你两层池子：

- `core_pool_*`
  - 通过硬过滤的钱包里，按分数排前 **最多 5 个**
- `active_pool_*`
  - 在 core pool 里，再只保留 **当前还有仓位** 的钱包，按分数取前 **最多 2 个**

也就是说：
> 现在不是只能给你一个 wallet，而是开始把“核心池 / 激活池”也顺手算出来了。

如果你看这些字段还是嫌抽象，可以直接看这份解释文档：

```bash
cat wallet_filter_metrics_explained.md
```

里面已经把 `score_total`、`maker_rebate_count`、`tail24`、`copyable_ratio`、`rejection_reasons` 这些指标全部翻成人话了。

另外现在 report 里还会多出：

- `review_status`
- `review_reasons`

这两个字段不是“这轮筛选过不过”，而是：

> **如果一个钱包已经进了你的长期观察池，现在是不是该降级 / 拉黑。**

### 7.1b 如果你想一口气扫多个类别

如果你不是只看一个类别，而是想看：

- `SPORTS`
- `POLITICS`
- `CRYPTO`
- 或整个 `SPECIALIST`

那就直接跑：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin scan_copy_leader_categories -- \
  --discovery-dir ../.omx/discovery \
  --categories SPECIALIST \
  --proxy http://127.0.0.1:7897 \
  --limit 1 \
  --connect-timeout-ms 5000 \
  --max-time-ms 12000
```

这个命令会：
- 按类别调用 `discover_copy_leader`
- 保留每个类别自己的 report
- 再生成一份总汇总：
  - `.omx/discovery/wallet-filter-v1-summary.txt`

你最该先看：

```bash
cat ../.omx/discovery/wallet-filter-v1-summary.txt
```

它会告诉你：
- 哪些 category 直接被拒了
- 哪些 category 通过了
- 每个 category 对应的 report 路径

而且 summary 现在还会直接给你这几组聚合指标：

- `best_rejected_*`
  - strict 下最接近通过的类别
- `best_watchlist_*`
  - strict 没过，但最值得继续盯的观察对象
- `watchlist_candidates`
  - 当前最值得盯的前几条 watchlist lane
- `closest_rejected_categories`
  - 当前最接近过 strict gate 的前几条 lane

也就是说：
> 现在不只是“扫完以后给你一堆 report”，而是已经会直接把最该关注的类别排出来。

比如会看到这种结构：

```text
== category SPORTS ==
status=rejected
error=...
report_path=../.omx/discovery/wallet-filter-v1-sports.txt
```

所以现在不是只能“逐个手工试 category”，而是已经有了**批量扫 smart-money category 池**的入口。

### 7.2 你也可以分开跑

抓榜：

```bash
cargo run --bin fetch_trader_leaderboard -- \
  --category OVERALL \
  --time-period DAY \
  --order-by PNL \
  --limit 20 \
  --proxy http://127.0.0.1:7897 \
  --output ../.omx/discovery/leaderboard-overall-day-pnl.json
```

抓某个用户 activity：

```bash
cargo run --bin fetch_user_activity -- \
  --user 0x你的wallet \
  --type TRADE \
  --limit 20 \
  --proxy http://127.0.0.1:7897 \
  --output ../.omx/discovery/activity-0x你的wallet-trade.json
```

用 leaderboard 结果选 leader：

```bash
cargo run --bin select_copy_leader -- \
  --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json \
  --output ../.omx/discovery/selected-leader.env
```

用 activity 结果选 leader：

```bash
cargo run --bin select_copy_leader -- \
  --activity ../.omx/discovery/activity-0x你的wallet-trade.json \
  --output ../.omx/discovery/selected-leader.env
```

---

## 8. 第三步：真实 watcher，盯 leader 的 activity

```bash
cd ~/onFire/rust-copytrader
cargo run --bin watch_copy_leader_activity -- \
  --root .. \
  --proxy http://127.0.0.1:7897 \
  --poll-count 1
```

成功后你会看到：
- `watch_user=0x...`
- `watch_latest_path=.../latest-activity.json`
- `watch_log_path=.../activity-events.jsonl`

落地文件：
- `.omx/live-activity/<wallet>/latest-activity.json`
- `.omx/live-activity/<wallet>/activity-events.jsonl`
- `.omx/live-activity/<wallet>/seen-tx.txt`

这一步的意义是：

> 你已经不是“拿静态榜单”了，而是在盯一个真实 leader 的公开 activity。

---

## 9. 第四步：把真实 activity 喂进 guarded runtime

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_guarded_cycle -- --root ..
```

成功后通常能看到：
- `cycle_outcome=processed`
- `runtime_mode=replay`
- `last_submit_status=verified`

这里要说清楚：

- 这一步是 **真实 activity 输入**
- 但 runtime 执行侧仍然是 **guarded / replay-backed**
- 它的作用是证明整条热路径在当前约束下能正确吃进去

不是说已经真下单了。

---

## 10. 第五步：跑到 live submit gate（默认安全）

### 10.1 单独跑 preview gate

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_live_submit_gate -- \
  --root .. \
  --activity-source-verified \
  --activity-under-budget \
  --activity-capability-detected \
  --positions-under-budget
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
- `unsigned_maker_amount=...`
- `unsigned_taker_amount=...`
- `preview_program=...`
- `preview_args=...`

---

## 11. 第六步：一键 operator flow（最推荐）

如果你不想一步一步敲，直接跑这条：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_operator_flow -- \
  --root .. \
  --discovery-dir ../.omx/discovery \
  --proxy http://127.0.0.1:7897 \
  --watch-poll-count 1 \
  --connect-timeout-ms 8000 \
  --max-time-ms 20000 \
  --live-submit-gate
```

它会串起来：

1. discovery
2. 选 leader
3. watcher
4. guarded cycle
5. live submit gate
6. operator demo report

最终会落：
- `.omx/operator-demo/discover-and-demo-*.txt`

你最该看这些字段：
 - `selected_category=...`
 - `selected_score=...`
- `selected_rank=...`
- `selected_pnl=...`
- `watch_user=...`
- `cycle_outcome=processed`
- `live_gate_status=unlocked`
- `live_submit_status=preview_only`

如果这些都在，就说明：

> **真实公开数据 -> 真实 leader -> 真实 watcher -> 真实 live preview**
> 已经串通了。

---

## 12. 第七步：自动循环 lane

如果你想让它不是只跑一轮，而是按 loop 跑：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_auto_guarded_loop -- \
  --root .. \
  --proxy http://127.0.0.1:7897 \
  --watch-poll-count 1 \
  --loop-count 1 \
  --live-submit-gate
```

会落：
- `.omx/auto-guarded/auto-guarded-*.txt`

这条 lane 当前也已经能跑到：
- `live_gate_status=unlocked`
- `live_submit_status=preview_only`

---

## 13. 真下单怎么做（先看再决定）

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
  --activity-source-verified \
  --activity-under-budget \
  --activity-capability-detected \
  --positions-under-budget \
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

### 15.1 Bootstrap / smoke

```bash
cd ~/onFire/rust-copytrader
cargo run -- --root ..
cargo run -- --smoke-helper --root ..
cargo run -- --smoke-runtime --root ..
cargo run -- --operator-demo --root ..
```

### 15.2 Discovery

```bash
cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --proxy http://127.0.0.1:7897
cargo run --bin fetch_user_activity -- --user 0xWALLET --type TRADE --limit 20 --proxy http://127.0.0.1:7897
cargo run --bin discover_copy_leader -- --discovery-dir ../.omx/discovery --proxy http://127.0.0.1:7897 --category SPECIALIST --connect-timeout-ms 8000 --max-time-ms 20000
```

### 15.3 Watch / guarded

```bash
cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --poll-count 1
cargo run --bin run_copytrader_guarded_cycle -- --root ..
cargo run --bin run_copytrader_auto_guarded_loop -- --root .. --proxy http://127.0.0.1:7897 --watch-poll-count 1 --loop-count 1 --live-submit-gate
```

### 15.4 Live preview / live submit

```bash
cargo run --bin run_copytrader_live_submit_gate -- --root .. --activity-source-verified --activity-under-budget --activity-capability-detected --positions-under-budget
cargo run --bin run_copytrader_live_submit_gate -- --root .. --activity-source-verified --activity-under-budget --activity-capability-detected --positions-under-budget --allow-live-submit
```

### 15.4b Position targeting demo

如果你想直接看：

- 选中的 leader
- positions / value
- 仓位目标引擎算出来的 target / delta

可以直接跑：

```bash
cargo run --bin run_position_targeting_demo -- --root ..
```

它会读取：
- `.omx/discovery/selected-leader.env`
- `.omx/discovery/positions-<wallet>.json`
- `.omx/discovery/value-<wallet>.json`
- `.omx/discovery/markets/*.json`

然后输出：
- `leader_position_count`
- `leader_spot_value_usdc`
- `leader_ewma_value_usdc`
- `target_count`
- `delta_count`
- `diagnostic_total_target_risk_usdc`

并且把报告落到：
- `.omx/position-targeting/position-targeting-*.txt`

### 15.4c ANSI 轻量终端面板

如果你不想一条条翻：

- `wallet-filter-v1-summary.txt`
- `selected-leader.env`
- `operator-demo/latest.txt`
- `position-targeting-*.txt`

那可以直接开一个轻量 ANSI 终端面板：

```bash
cargo run --bin run_copytrader_ansi_dashboard -- --root ..
```

它会用 ANSI 清屏重绘，默认每秒刷新一次，主要展示：

- smart-money summary
- selected leader
- operator lane
- position targeting
- auto-guarded 最新报告

如果你只想看一帧，不要持续刷新：

```bash
cargo run --bin run_copytrader_ansi_dashboard -- --root .. --once
```

如果想调刷新频率：

```bash
cargo run --bin run_copytrader_ansi_dashboard -- --root .. --interval-ms 500
```

### 15.5 One-command operator flow

```bash
cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery --proxy http://127.0.0.1:7897 --watch-poll-count 1 --connect-timeout-ms 8000 --max-time-ms 20000 --live-submit-gate
```

---

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
cargo test --bin run_copytrader_operator_flow
cargo test --bin run_copytrader_auto_guarded_loop
cargo test --bin run_copytrader_live_submit_gate
```

### 16.3 Python helper / shell wrapper

```bash
cd ~/onFire
PYTHONPATH=polymarket_arb $HOME/anaconda3/bin/python3 -m unittest \
  scripts.tests.test_clob_sign_helpers \
  scripts.tests.test_run_rust_helper_smoke \
  scripts.tests.test_run_rust_operator_demo \
  scripts.tests.test_run_rust_runtime_smoke
```

### 16.4 文档相关测试

```bash
cd ~/onFire
PYTHONPATH=polymarket_arb $HOME/anaconda3/bin/python3 -m pytest -q tests/test_secret_setup_doc.py
```

---

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
- 先跑：

```bash
cargo run -- --smoke-helper --root ..
```

如果 helper smoke 都过不了，再看 Python SDK / key 配置。

---

### 17.3 `Using SOCKS proxy, but the 'socksio' package is not installed`
这通常是全局 `all_proxy=socks5://...` 影响到了本地 helper。

当前仓库已经尽量隔离这个问题。
如果你还碰到：
- discovery/watch 继续用显式 `--proxy`
- 不要指望 signing helper 靠 SOCKS 环境工作

---

### 17.4 `py-clob-client / py-order-utils import failed`
说明本地 Python SDK 依赖不完整。

处理：
- 确认当前 `python3` / `PYTHON_BIN` 对应的是你装过 SDK 的环境
- 最稳妥是用：

```bash
export PYTHON_BIN=$HOME/anaconda3/bin/python3
```

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

> **只差你显式执行 `--allow-live-submit` 这一步。**

也就是说：
- discovery 已经真连了
- watcher 已经真连了
- guarded cycle 已经真吃到 activity 了
- submit preview 已经真生成了
- 剩下就是真正放开 live submit

---

## 19. 我建议你怎么实际使用

### 如果你现在只是想确认“整条链通没通”
直接跑：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_operator_flow -- \
  --root .. \
  --discovery-dir ../.omx/discovery \
  --proxy http://127.0.0.1:7897 \
  --watch-poll-count 1 \
  --connect-timeout-ms 8000 \
  --max-time-ms 20000 \
  --live-submit-gate
```

### 如果你想持续看自动链路是否还稳定
跑：

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_auto_guarded_loop -- \
  --root .. \
  --proxy http://127.0.0.1:7897 \
  --watch-poll-count 1 \
  --loop-count 1 \
  --live-submit-gate
```

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
