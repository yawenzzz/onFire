# monitor_v1 说明

这个文件专门解释：

- `run_copytrader_monitor_v1` 到底在干什么
- 现在哪些数据是真实抓的，哪些是 shadow / 推导出来的
- ANSI 面板每个区块是什么意思
- 为什么会出现 `delta_count=0`

---

## 1. 入口

```bash
cd ~/onFire/rust-copytrader
cargo run --bin run_copytrader_monitor_v1 -- --root .. --proxy http://127.0.0.1:7897
```

只看一帧：

```bash
cargo run --bin run_copytrader_monitor_v1 -- \
  --root .. \
  --iterations 1 \
  --once \
  --no-http \
  --proxy http://127.0.0.1:7897
```

默认 HTTP：

- `http://127.0.0.1:9911/healthz`
- `http://127.0.0.1:9911/readyz`
- `http://127.0.0.1:9911/metrics`

---

## 2. 这版 monitor v1 的定位

当前这版是：

> **smart-money 跟踪 + shadow-poll monitor lane**

不是：

- 真正接管 live submit 的总控
- 全量 market ws / user ws 的实盘监控器
- Prometheus / Grafana 重依赖方案

当前 monitor v1 主要做四件事：

1. 从 `.omx/discovery/selected-leader.env` 读取当前被选中的 smart-money 钱包
2. 实时轮询这个钱包的最新公开 activity
3. 周期性运行 `run_position_targeting_demo`
4. 把结果汇总成 ANSI / JSONL / `healthz` / `readyz` / `metrics`

---

## 3. 哪些数据是真实的，哪些是推导的

### 真实抓取的
- selected leader 信息
- leader 最新公开 activity
- discovery 目录下缓存的 positions / value
- operator demo / guarded lane 已落盘的 artifact

### 本地推导的
- `book` 区块里现在很多是 synthetic book 视图
- `execution` 区块现在优先复用 operator artifact，不是 live websocket 全链路回报
- `position targeting` 区块里的 blocker summary 是 **probable blockers**，是为了帮助解释为什么当前算不出 delta

所以要把它理解成：

> **实时监控面板 + 解释面板**

而不是“交易所官方状态真相屏”。

---

## 4. ANSI 面板怎么读

### 4.1 顶栏
- `HEALTH`
  - `OK / WARN / CRIT`
- `equity`
  - 当前 leader 总 value 的近似镜像
- `cash`
  - `equity - deployed`
- `deployed`
  - 当前仓位总暴露
- `loop_lag_p95`
  - monitor 自己这条循环的延迟
- `mon_drop`
  - monitor channel 丢事件数量
- `rss / fds / threads`
  - 当前 monitor 进程的轻量进程视图
  - 这版是 best-effort 采样，不保证像专业 profiler 一样精准

---

### 4.2 feeds
这块看外部依赖是不是健康。

当前你最该看：

- `data_api p95`
  - activity / positions / value 相关请求耗时
- `429_1m`
  - 最近一分钟是否被限流
- `market_ws / user_ws`
  - 当前 monitor v1 是 shadow 模式，所以会显示 `shadow_poll`

---

### 4.3 selected leader
这块回答的是：

> **现在 monitor 盯的是谁，它是怎么被选出来的**

字段：

- `wallet`
  - 当前被盯的钱包地址
- `category`
  - 来源类别，比如 `TECH`
- `score`
  - wallet_filter_v1 的得分
- `review`
  - `stable / downgrade / blacklist`
- `source`
  - 选中来源，例如：
    - `wallet_filter_summary:...#best_watchlist_wallet`
- `core_pool / active_pool`
  - 如果 strict pass 池里有结果，这里会显示池子内容；否则通常是 `none`

---

### 4.4 tracked activity
这块回答的是：

> **这个 smart-money 钱包最近一笔公开 activity 到底是什么**

字段：

- `tx`
  - 最近一笔 activity 的交易哈希
- `side`
  - `BUY / SELL`
- `slug`
  - 对应市场 slug
- `asset`
  - 对应 asset id
- `usdc`
  - 这笔 activity 的 USDC 规模
- `event_age`
  - 这笔 activity 离现在多久

这块是最直接的“实时追踪”面。

---

### 4.5 leaders
这块回答的是：

> **这个 leader 最近活动新不新、对账慢不慢、持仓大不大**

字段：

- `activity_p95`
  - 最新 activity 事件年龄
- `snap_age`
  - 最近 reconcile 的快照年龄
- `reconcile_p95`
  - position targeting / reconcile 这一层耗时
- `drift_p95`
  - provisional 和 snapshot 的漂移
- `positions`
  - 当前缓存持仓数
- `value`
  - 当前 leader value

---

### 4.6 books
这块看当前 monitor 用来估算可执行性的 book 视图。

当前这版很多是 synthetic book，所以重点是：

- `spread`
- `levels`
- `resync_5m`
- `crossed`
- `hash_mismatch`

后面如果把真实 market ws 接进来，这块会更有价值。

---

### 4.7 signals
这块看目标仓位的方向和新鲜度。

- `raw`
  - 原始目标风险金额
- `final`
  - 经约束投影后的目标风险金额
- `agree`
  - leader 之间一致性（当前单 leader 基本就是 100%）
- `fresh`
  - 信号新鲜度
- `SKIP ...`
  - 说明该资产被规则跳过了

---

### 4.8 position targeting
这是这次新增的重要区块。

字段：

- `target_count`
  - 生成了多少个 target
- `delta_count`
  - 最终有多少个可执行 delta
- `stale_assets`
  - stale target 数量
- `blocked_assets`
  - 被压成 0 的资产数量
- `blocker_summary`
  - 当前最主要的 **probable blockers** 汇总

示例：

```text
blocker_summary=zero_target:188,tail_lt24h:91,neg_risk:79,low_copyable_liquidity:25
```

这个意思是：

- 很多 target 已经被压成 0
- 相当多资产已经进入尾盘窗口
- 有不少是 negRisk 市场
- 还有一部分复制流动性不够

---

### 4.9 execution
这块不是实时 user ws 成交真相，而是当前 operator / guarded artifact 的执行摘要。

重点看：

- `copy_gap p95`
- `slip p95`
- `fee_adj_slip p95`
- `last_submit`

---

### 4.10 risk
这块是当前 leader 持仓画像。

字段：

- `gross / net`
- `tail<24h / tail<72h`
- `negRisk`
- `track_err / rmse_1m`
- `follow_ratio`

如果这里 `negRisk` 很高，而 `alerts` 里也有：

- `neg_risk_exposure_present`

那说明当前这条 leader 的结构并不适合直接复制。

--- 

### 4.11 alerts
这里就是系统当前判出来的 WARN / CRIT。

例如：

- `neg_risk_exposure_present`
- `copy_gap_wide`
- `main_loop_lag`
- `market_ws_stale`

---

## 5. 为什么会出现 `delta_count=0`

这是最近最容易误解的一点。

`delta_count=0` **不等于没有仓位**。

它更常见的意思是：

> **leader 有仓位，但按当前 position_v1 约束，你不应该继续开新仓跟。**

常见原因：

1. `zero_target`
   - 聚合后的 target 已经被压成 0
2. `tail_lt24h`
   - 太接近结算，不开新仓
3. `tail_lt72h`
   - 已经进入缩仓区间
4. `neg_risk`
   - negRisk 市场默认不新开
5. `low_copyable_liquidity`
   - 流动性 / 24h 成交不够
6. `below_min_effective_order`
   - target 还在，但太小，不值得下
7. `stale_target`
   - target 已经过时

所以以后你看到：

- `positions=190`
- `delta_count=0`

不要再理解成“没仓位”，而要理解成：

> **有仓位，但当前规则认为不该跟。**

---

## 6. 这版 monitor v1 还没做完的部分

### 已经有的
- smart-money selected leader 上下文
- 实时 activity 跟踪
- position targeting 汇总
- blocker summary
- ANSI 界面
- JSONL journal
- `/healthz /readyz /metrics`

### 还没有的
- 真正的 market websocket 实盘订阅
- 真正的 user websocket 实盘回报
- 多 leader 并行组合 monitor
- 长时间运行的自动 leader 热切换策略

---

## 7. 你现在最推荐怎么用

### 先单次看一眼
```bash
cargo run --bin run_copytrader_monitor_v1 -- \
  --root .. \
  --iterations 1 \
  --once \
  --no-http \
  --proxy http://127.0.0.1:7897
```

### 再持续跑
```bash
cargo run --bin run_copytrader_monitor_v1 -- --root .. --proxy http://127.0.0.1:7897
```

### 同时开 HTTP
默认就开：
- `127.0.0.1:9911/healthz`
- `127.0.0.1:9911/readyz`
- `127.0.0.1:9911/metrics`

---

## 8. 一句话总结

`run_copytrader_monitor_v1` 现在的定位是：

> **把 smart-money 钱包筛选、activity 跟踪、position targeting、执行摘要、风险画像，整成一个能实时看的轻量 monitor。**

如果下一步要继续增强，最该补的是：

1. 真实 market/user websocket 接入
2. delta blocker 的更细粒度因果归因
3. 多 leader 同屏与组合级监控
