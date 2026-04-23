# rust-copytrader README

这个仓库现在的重点不是“概念验证”，而是**围绕 Polymarket leader activity 的实用跟单脚本集**：

- 抓 leader 最新动作
- 用当前持仓过滤“是否真的是新开仓”
- 生成/提交跟单订单
- 实时记录 follower 真实成交延时与价差
- 监控 follower 账户状态

如果你只想马上跑起来，先看这三个入口：

```bash
# 1) 连续跟单（实时提交）
bash scripts/run_rust_minmax_follow_live_submit.sh --user <leader_wallet>

# 2) 连续跟单 + 真实成交延时日志
bash scripts/run_rust_minmax_follow_live_submit_latency.sh --user <leader_wallet>

# 3) 单次强制处理 leader 最新动作
bash scripts/run_rust_follow_last_action_force_live_once.sh --user <leader_wallet>
```

---

## 1. 目录与输出

运行产物主要在仓库根目录 `.omx/` 下：

- `.omx/live-activity/<leader_wallet>/`
  - `latest-activity.json`：leader 最新动作
  - `activity-events.jsonl`：leader 动作流
  - `seen-tx.txt`：已处理 tx
- `.omx/force-live-follow/<leader_wallet>/runs/<run_id>/`
  - `summary.txt`：一次 force-follow 运行的完整 summary
  - `watch.stdout.log` / `submit.stdout.log`
- `.omx/live-submit/`
  - live submit gate 报告
- `.omx/fill-latency/<leader_wallet>/`
  - `.omx/fill-latency/<leader_wallet>/fills.log`：简洁实时成交日志
  - `.omx/fill-latency/<leader_wallet>/fills.jsonl`：结构化成交日志
  - `follow.stdout.log` / `follow.stderr.log`
- `.omx/account-monitor/`
  - 账户监控输出

---

## 2. 环境准备

至少准备：

- Rust / Cargo
- Polymarket 所需认证信息（建议放到 repo 根目录 `.env` 或 `.env.local`）

常见变量：

```bash
POLY_ADDRESS=0x...
CLOB_API_KEY=...
CLOB_SECRET=...
CLOB_PASS_PHRASE=...
PRIVATE_KEY=...
SIGNATURE_TYPE=0
FUNDER_ADDRESS=0x...
CLOB_HOST=https://clob.polymarket.com
COPYTRADER_MAX_TOTAL_EXPOSURE_USDC=100
COPYTRADER_MAX_ORDER_USDC=10
COPYTRADER_ACCOUNT_SNAPSHOT_PATH=runtime-verify-account/dashboard.json
COPYTRADER_ACCOUNT_SNAPSHOT_MAX_AGE_SECS=300
COPYTRADER_ACTIVITY_MAX_AGE_SECS=60
```

如果要走 websocket 版 leader watcher，还要准备：

```bash
POLYGON_WSS_URL=wss://<your_polygon_ws>
```

如果你的网络需要代理：

```bash
export POLYMARKET_CURL_PROXY=http://127.0.0.1:7897
```

也可以在脚本上显式传：

```bash
--proxy http://127.0.0.1:7897
```

---

## 3. 推荐工作流

### 3.1 只看 leader 最新动作

```bash
bash scripts/run_rust_watch_copy_leader_activity.sh --root .. --user <leader_wallet> --poll-count 1
```

更低延迟 websocket 版本：

```bash
bash scripts/run_rust_watch_copy_leader_activity_ws.sh --root .. --user <leader_wallet> --poll-count 1
```

### 3.2 跑一轮 minmax 跟单决策，但不一定真下单

```bash
bash scripts/run_rust_minmax_follow_live.sh --user <leader_wallet>
```

如果要连续跑：

```bash
FOLLOW_FOREVER=1 bash scripts/run_rust_minmax_follow_live.sh --user <leader_wallet>
```

### 3.3 连续跟单提交

```bash
bash scripts/run_rust_minmax_follow_live_submit.sh --user <leader_wallet>
```

这条链路会：

1. 持续盯 leader 最新动作
2. 只在 leader **新开事件仓位**时才跟
3. 首笔/补仓都按 `leader shares / 10`
4. `< 5 shares` 直接跳过
5. BUY 跟单走 `GTC limit order`

### 3.4 连续跟单 + 实时成交延时日志

```bash
bash scripts/run_rust_minmax_follow_live_submit_latency.sh --user <leader_wallet>
```

这是当前最推荐的实盘观察入口。它会同时：

- 跑连续跟单
- 监听 follower 自己账户 websocket 成交事件
- 实时输出简洁日志
- 落文件到 `.omx/fill-latency/<leader_wallet>/`

注意：这条脚本里的 **fill latency logger 会登录 follower 自己的 authenticated websocket**，所以除了 `--user <leader_wallet>` 之外，还必须在本地 `.env` / `.env.local` 或进程环境里提供 follower 账户的：

- `PRIVATE_KEY` 或 `CLOB_PRIVATE_KEY`
- `CLOB_API_KEY`
- `CLOB_SECRET`
- `CLOB_PASS_PHRASE`

日志形态示例：

```text
fill latency_ms=1234 leader_ts_ms=... fill_ts_ms=... leader_price=0.50000000 fill_price=0.50100000 price_gap_bps=20.0000 shares=6.00 requested_shares=6.00 leader_tx=0x... trade_id=...
```

---

## 4. 核心脚本说明

### A. Leader activity 相关

| 脚本 | 用途 | 常见用法 |
| --- | --- | --- |
| `scripts/run_rust_watch_copy_leader_activity.sh` | HTTP 轮询 leader activity | `bash scripts/run_rust_watch_copy_leader_activity.sh --user <leader_wallet> --poll-count 1` |
| `scripts/run_rust_watch_copy_leader_activity_ws.sh` | WebSocket 版 leader watcher | `bash scripts/run_rust_watch_copy_leader_activity_ws.sh --user <leader_wallet> --poll-count 1` |

### B. 跟单决策 / 提交相关

| 脚本 | 用途 | 常见用法 |
| --- | --- | --- |
| `scripts/run_rust_minmax_follow.sh` | Rust minmax 跟单策略主入口 | `bash scripts/run_rust_minmax_follow.sh --user <leader_wallet> --loop-count 1` |
| `scripts/run_rust_minmax_follow_live.sh` | 带默认风险参数的 live 跟单包装 | `bash scripts/run_rust_minmax_follow_live.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_live_ws.sh` | 把 live 跟单切到 WS watcher | `bash scripts/run_rust_minmax_follow_live_ws.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_live_submit.sh` | 连续自动跟单提交 | `bash scripts/run_rust_minmax_follow_live_submit.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_live_submit_ws.sh` | 连续自动跟单提交 + WS watcher | `bash scripts/run_rust_minmax_follow_live_submit_ws.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_live_submit_once.sh` | 只处理一次最新新动作 | `bash scripts/run_rust_minmax_follow_live_submit_once.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_force_live_once.sh` | 强制 live submit 一轮（忽略 seen） | `bash scripts/run_rust_minmax_follow_force_live_once.sh --user <leader_wallet>` |
| `scripts/run_rust_follow_last_action_force_live_once.sh` | 处理 leader 最新动作的底层 force-follow 主脚本 | `bash scripts/run_rust_follow_last_action_force_live_once.sh --user <leader_wallet>` |

### C. 下单 / 持仓 gate 相关

| 脚本 | 用途 | 常见用法 |
| --- | --- | --- |
| `scripts/run_rust_live_submit_gate.sh` | 把选中的 latest activity 转成 preview/live submit | `bash scripts/run_rust_live_submit_gate.sh --root .. --latest-activity <path> --selected-leader-env <path>` |
| `scripts/run_rust_public_positions_gate.sh` | 查询目标 wallet 当前持仓，确认是不是“新开仓” | `bash scripts/run_rust_public_positions_gate.sh --user <leader_wallet> --latest-activity <selected_latest_activity.json>` |
| `scripts/run_rust_ctf_action.sh` | 处理 `MERGE` / `SPLIT` CTF 动作 | `bash scripts/run_rust_ctf_action.sh --root .. --latest-activity <path> --selected-leader-env <path>` |

### D. 延时 / 成交日志相关

| 脚本 | 用途 | 常见用法 |
| --- | --- | --- |
| `scripts/run_rust_copytrade_latency_report.sh` | 读取现有 summary，输出 submit 路延时报告 | `bash scripts/run_rust_copytrade_latency_report.sh --user <leader_wallet> --source force-live` |
| `scripts/run_rust_copytrade_fill_latency_logger.sh` | 单独启动 follower 成交 websocket logger | `bash scripts/run_rust_copytrade_fill_latency_logger.sh --user <leader_wallet>` |
| `scripts/run_rust_minmax_follow_live_submit_latency.sh` | 连续跟单 + 真实成交延时日志的一体化入口 | `bash scripts/run_rust_minmax_follow_live_submit_latency.sh --user <leader_wallet>` |

### E. follower 账户监控相关

| 脚本 | 用途 | 常见用法 |
| --- | --- | --- |
| `scripts/run_rust_show_account_info.sh` | 拉一次账户快照 | `bash scripts/run_rust_show_account_info.sh --output runtime-verify-account/dashboard.json` |
| `scripts/run_rust_account_monitor.sh` | 持续轮询 follower 账户状态 | `bash scripts/run_rust_account_monitor.sh --output .omx/account-monitor/latest.json` |
| `scripts/run_rust_account_user_ws.sh` | 监听 follower 自己账户 websocket 事件 | `bash scripts/run_rust_account_user_ws.sh --output .omx/account-monitor/user-ws.json` |

---

## 5. 关键行为说明

### 5.1 什么时候会跟单

当前 force-follow / auto-follow 路径只会在以下条件同时满足时跟单：

- latest activity 是 `TRADE`
- side 是 `BUY`
- `public positions gate` 确认 leader 当前持仓表明这是**新开仓**
- 按 `leader shares / 10` 计算后的 shares **>= 5**

否则会 fail-closed 跳过，并把原因写进 summary。

### 5.2 什么时候会打出 fill latency 日志

只有在 follower 账户 websocket 收到**真实 trade fill** 时才会记日志。

所以：

- submit 成功但单子还挂着，没有成交 -> **不会有 fill 日志**
- limit 单分多次成交 -> **会打多条 fill 日志**

### 5.3 延时是怎么算的

`fill latency` 使用：

- `leader latest activity timestamp`
- 对比 follower websocket trade 事件里的：
  - `matchtime`（优先）
  - 否则 `timestamp`
  - 再否则 `last_update`

因此它表示的是：

> **leader 动作发生时间 -> follower 真实成交时间**

不是 submit 请求结束时间。

---

## 6. 最常用命令速查

### 连续自动跟单

```bash
bash scripts/run_rust_minmax_follow_live_submit.sh --user <leader_wallet>
```

### 连续自动跟单 + 实时成交延时日志

```bash
bash scripts/run_rust_minmax_follow_live_submit_latency.sh --user <leader_wallet>
```

### 单次处理 leader 最新动作

```bash
bash scripts/run_rust_follow_last_action_force_live_once.sh --user <leader_wallet>
```

### 看 force-follow 最近一次 summary

```bash
bash scripts/run_rust_copytrade_latency_report.sh --user <leader_wallet> --source force-live
```

### 单独看 follower 账户 websocket

```bash
bash scripts/run_rust_account_user_ws.sh --output .omx/account-monitor/user-ws.json
```

---

## 7. 补充文档

如果你要看更细的专项文档：

- 账户监控：`rust-copytrader/ACCOUNT_MONITOR.md`
- submit 路延时字段：`rust-copytrader/COPYTRADE_LATENCY.md`

---

## 8. 当前建议

如果你现在只保留一个“日常实用入口”，就用：

```bash
bash scripts/run_rust_minmax_follow_live_submit_latency.sh --user <leader_wallet>
```

因为它同时覆盖：

- 连续跟单
- 当前持仓过滤
- limit 跟单
- 真实成交延时
- 持久化日志
