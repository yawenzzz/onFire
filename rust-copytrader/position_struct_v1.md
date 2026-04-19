下面这版是能直接丢给工程同学实现的规格。

Polymarket 这套系统天然适合做成“**Data API 触发 + positions 快照纠偏 + market WS 定价 + user WS 回报**”的四段式。官方现在把公开发现/画像数据放在 Gamma/Data API，把盘口和下单放在 CLOB，market WebSocket 是公开的，user WebSocket 只给你自己订单和成交；官方也提供 Rust SDK 做完整 CLOB 认证和交易。([Polymarket 文档][1])

---

## 0. 先定目标

你要解的不是“看到 leader 一笔成交，我抄多少”，而是：

**在低延时前提下，让我的组合尽可能逼近 leader 组合，同时扣掉成交成本、滑点、延时衰减、尾盘惩罚和风险上限。**

所以主问题写成：

[
\delta^*=\arg\min_{\delta}
\Big[
\underbrace{\sum_a w_a,(q_a+\delta_a-\hat q_a)^2}*{跟踪误差}
+
\underbrace{\lambda*{tc}\sum_a C_a(\delta_a)}*{成交成本}
+
\underbrace{\lambda*{churn}\sum_a |\delta_a|}*{换手惩罚}
+
\underbrace{\lambda*{risk}R(q+\delta)}_{事件/到期/集中度风险}
\Big]
]

其中：

* (a) 是 **asset / token**
* (q_a) 是你当前持仓的“风险资本”
* (\hat q_a) 是 leader 聚合后的目标风险资本
* (C_a(\delta)) 是用真实 orderbook walk 出来的分段线性成本
* (R(\cdot)) 是单市场、单事件、尾盘、negRisk、现金占用这些约束的惩罚项

**关键设计选择：用“风险资本 USDC”做主单位，不用 shares 做主单位。**

原因有两个：

第一，Polymarket 的 BUY market order 本来就是按美元金额下，SELL 才按 shares 下。第二，leader 的 `/activity` 直接给 `usdcSize`，`/positions` 也直接给 `initialValue`、`currentValue`，用风险资本统一最自然。官方下单语义也是这样：FAK/FOK 里 BUY 指定花多少钱，SELL 指定卖多少 shares。([Polymarket 文档][2])

---

## 1. 外部输入 contract

### 1.1 必用的外部数据源

**触发器**

* `GET /activity`
* 只看 `type=TRADE`
* 用 `start` 做增量轮询
* 用 `(leader, transactionHash, asset, side, size)` 去重

`/activity` 会返回 `timestamp`、`conditionId`、`type`、`usdcSize`、`transactionHash`、`price`、`asset`、`side`、`slug`、`outcome` 等。它还会包含 `SPLIT / MERGE / CONVERSION / MAKER_REBATE` 等类型，所以不能把全部 activity 都当方向信号。([Polymarket 文档][3])

**真相快照**

* `GET /positions?user=...&sizeThreshold=0`
* `GET /value?user=...`

`/positions` 默认 `sizeThreshold=1`，不改成 `0` 会漏掉小仓位。`/value` 给你 leader 组合总值，用来做归一化和 EWMA。([Polymarket 文档][4])

**实时定价**

* market WebSocket 订阅活跃 asset
* 冷启动/容错用 `POST /books`

官方文档明确把 market channel 定义成公开的实时订单簿/价格/市场生命周期流；批量 `/books` 可拿多 token 书，orderbook 文档说明 batch 最多 500 个 token，实时场景优先用 WS 不要轮询。([Polymarket 文档][5])

**我方回报**

* user WebSocket
* 只用于我自己的 order/trade 状态

user channel 是认证通道，回报包括 trade/order 事件；trade 状态按 `MATCHED -> MINED -> CONFIRMED` 演进，也可能出现 `RETRYING`、`FAILED`。([Polymarket 文档][6])

**市场元数据**

* market object：`endDate`、`acceptingOrders`、`enableOrderBook`、`liquidityClob`、`volume24hrClob`、`negRisk`、`clobTokenIds`

这些字段决定尾盘惩罚、能不能下单、是否要禁做 negRisk 新开仓、以及 copyable liquidity。([Polymarket 文档][7])

### 1.2 限流预算

官方当前限流足够做轻量单进程架构：Data API 通用 `1000 req / 10s`，`/positions` `150 req / 10s`；CLOB `/books` `500 req / 10s`，`/book` `1500 req / 10s`；交易端点另有 burst/sustained 限制。([Polymarket 文档][8])

工程上直接用两个 token bucket：

* `data_bucket`: 900 req / 10s 软上限
* `positions_bucket`: 120 req / 10s 软上限
* `clob_bucket`: 400 req / 10s 软上限

留冗余，别顶满官方上限。

---

## 2. 内部 canonical model

### 2.1 不要用 float

热路径不用 `f64`。

建议全程 fixed-point：

```rust
type UsdcMicros = i64;      // 1 USDC = 1_000_000
type SharesMicros = i64;    // 1 share = 1_000_000
type PricePpm = i32;        // 1.0 = 1_000_000
type Bps = i32;             // 1 bp = 0.01%
type UnixMs = i64;
```

原因：

* 价格最小 tick 可能到 `0.0001`
* 费用按 USDC 计算，四舍五入到 5 位小数，最小费用 `0.00001` USDC
* 你要做 book walk 和 fee walk，float 很容易把边界单搞错。([Polymarket 文档][9])

### 2.2 状态实体

```rust
struct LeaderId(String);
struct AssetId(String);
struct ConditionId(String);
struct EventId(String);

struct LeaderConfig {
    leader: LeaderId,
    base_score_bps: u32,      // 来自你前面的 leader 筛选器
    alpha_bps: u32,           // 单 leader 复制系数
    enabled: bool,
}

struct LeaderValue {
    spot_value: UsdcMicros,
    ewma_value: UsdcMicros,
    last_update_ms: UnixMs,
}

struct LeaderPosition {
    asset: AssetId,
    condition: ConditionId,
    event: Option<EventId>,
    outcome: String,              // "Yes"/"No"/others
    size: SharesMicros,
    avg_price_ppm: PricePpm,
    initial_value: UsdcMicros,
    current_value: UsdcMicros,
    end_ts_ms: UnixMs,
    neg_risk: bool,
    slug: String,
}

struct ProvisionalDelta {
    leader: LeaderId,
    asset: AssetId,
    signed_risk_usdc: i64,
    leader_event_ts_ms: UnixMs,
    local_recv_ts_ms: UnixMs,
    expires_at_ms: UnixMs,
    tx_hash: String,
}

struct BookLevel {
    price_ppm: PricePpm,
    size_shares: SharesMicros,
}

struct BookView {
    asset: AssetId,
    bids: smallvec::SmallVec<[BookLevel; 32]>,
    asks: smallvec::SmallVec<[BookLevel; 32]>,
    tick_size_ppm: PricePpm,
    min_order_size_shares: SharesMicros,
    last_trade_price_ppm: PricePpm,
    last_update_ms: UnixMs,
    hash: String,
}

struct MarketMeta {
    asset: AssetId,
    condition: ConditionId,
    event: Option<EventId>,
    end_ts_ms: UnixMs,
    accepting_orders: bool,
    enable_order_book: bool,
    liquidity_clob: UsdcMicros,
    volume_24h_clob: UsdcMicros,
    neg_risk: bool,
}

struct OwnPosition {
    asset: AssetId,
    shares: SharesMicros,
    avg_price_ppm: PricePpm,
    risk_usdc: UsdcMicros,     // 已投入资本 / 剩余风险资本
    current_value: UsdcMicros,
}

struct Target {
    asset: AssetId,
    signed_target_risk_usdc: i64,
    confidence_bps: u16,
    source_count: u8,
    stale: bool,
}
```

---

## 3. leader 信号建模

## 3.1 “快信号”和“真信号”分层

### 快信号：activity provisional delta

收到 leader 的 `TRADE` 时，先生成一个临时增量：

[
\Delta \tilde x_{i,a}^{fast}
============================

s(a,e)\cdot
\frac{\text{usdcSize}*e}{\widetilde V_i}
\cdot
\exp\left(-\frac{t*{now}-t_e}{\tau_{fast}}\right)
]

其中：

* (s(a,e)\in{-1,+1}) 是方向符号
* (\widetilde V_i) 是 leader 总值的 EWMA
* (\tau_{fast}) 建议 15–30 秒

方向符号定义：

* binary 市场：

  * BUY YES = `+1`
  * SELL YES = `-1`
  * BUY NO = `-1`
  * SELL NO = `+1`
* 非 binary / outcome 不是 YES/NO：

  * 不做 signed collapse
  * 直接按 asset 独立跟踪

### 真信号：positions snapshot

一旦 leader 被标记 dirty，就拉 `/positions?sizeThreshold=0` + `/value`，重建它的真实净仓：

[
x_{i,a}=
\frac{\sigma_a \cdot K_{i,a}}{\widetilde V_i + \varepsilon}
]

其中：

* (\sigma_a) 是方向
* (K_{i,a}) 优先用 `initialValue`
* 若 `initialValue` 不可信或缺失，用 `size * avgPrice`
* (\widetilde V_i) 用 EWMA(`/value`)

**规则：snapshot 永远覆盖 provisional。**

也就是说，fast path 只是抢时间，不是最终真相。

## 3.2 为什么用 `initialValue` 不用 `currentValue`

因为你要复制的是 leader 的**资本配置决策**，不是 mark-to-market 后的浮盈浮亏。

如果 leader 什么都没做，只是价格自己涨了，`currentValue` 会变大；如果你用它当目标，你会在 leader 没加仓时被迫追涨。
所以：

* **目标权重** 用 `initialValue / ewma_value`
* **风险监控** 用 `currentValue`
* **持仓展示** 两个都保留

---

## 4. 多 leader 聚合

### 4.1 leader 在线有效权重

[
r_i=
\underbrace{\frac{\text{base_score}*i}{10,000}}*{\text{离线选人分}}
\cdot
\underbrace{\frac{\alpha_i}{10,000}}*{\text{复制系数}}
\cdot
\underbrace{e^{-lag_i/\tau*{lag}}}*{\text{延时惩罚}}
\cdot
\underbrace{e^{-stale_i/\tau*{stale}}}*{\text{陈旧惩罚}}
\cdot
\underbrace{\frac{1}{1+\lambda*\rho \bar\rho_i}}_{\text{相关性惩罚}}
]

* `base_score_i`：你前面的 leader 评分器给
* `alpha_i`：每个 leader 的复制强度
* `lag_i`：activity 到本地的端到端延时
* `stale_i`：上次 positions snapshot 到现在多久
* (\bar\rho_i)：和其他 leader 的暴露相关性均值

默认值：

* `tau_lag = 3000ms`
* `tau_stale = 90s`
* `lambda_rho = 0.5`

### 4.2 每个 asset 的聚合暴露

先拿每个 leader 对这个 asset 的暴露：

[
z_{i,a} = x_{i,a} + \Delta \tilde x_{i,a}^{fast}
]

再做稳健聚合：

[
\bar z_a = \mathrm{HuberMean}*i(r_i z*{i,a})
]

如果你想先上最稳、最轻的版本，不用 Huber，也可以：

[
\bar z_a = \frac{\sum_i r_i z_{i,a}}{\sum_i r_i + \varepsilon}
]

### 4.3 一致性分数

[
agree_a=
\frac{
\left|
\sum_i r_i \cdot sign(z_{i,a}) \cdot \mathbf 1(|z_{i,a}|>\epsilon)
\right|
}{
\sum_i r_i \cdot \mathbf 1(|z_{i,a}|>\epsilon) + \varepsilon
}
]

* `agree_a = 1`：所有 leader 同方向
* `agree_a = 0`：完全对冲

这个量很重要。它决定你应该放大、缩小，还是直接不跟。

### 4.4 原始目标权重

[
\hat w_a^{raw}
==============

\kappa \cdot \bar z_a
\cdot
agree_a^{\alpha}
\cdot
fresh_a
\cdot
expiry_a
\cdot
copyable_a
]

其中：

* (\kappa)：全局风险杠杆，默认 `0.6`
* (\alpha)：一致性放大，默认 `1.5`
* `fresh_a`：信号新鲜度
* `expiry_a`：尾盘惩罚
* `copyable_a`：流动性/可复制性因子

然后：

[
\hat q_a^{raw}=E_t\cdot \hat w_a^{raw}
]

`E_t` 是你当前净值。

---

## 5. 三个最关键的因子

## 5.1 新鲜度因子

[
fresh_a = \exp\left(-\frac{\Delta t_a}{\tau_{sig}}\right)
]

* `Δt_a`：这个 asset 最近一次 leader 有效变动到现在的时间
* `tau_sig`：建议 45–90 秒

**经验规则：**

* `Δt > 180s` 不再开新仓，只允许减仓
* `Δt > 600s` 不做任何跟单动作

## 5.2 尾盘因子

[
expiry_a =
\begin{cases}
0, & tte_a < 24h \
0.5, & 24h \le tte_a < 72h \
1, & tte_a \ge 72h
\end{cases}
]

其中 `tte_a = endDate - now`。
市场对象本来就给 `endDate`。([Polymarket 文档][7])

更平滑一点也可以：

[
expiry_a =
\mathrm{clip}
\left(
\frac{tte_a - 24h}{72h-24h}, 0, 1
\right)
]

## 5.3 可复制性因子

[
copyable_a =
I(\text{acceptingOrders})
\cdot
I(\text{enableOrderBook})
\cdot
L_a
]

其中：

[
L_a=
\mathrm{clip}\left(\frac{\log(1+\text{liquidityClob})-\ell_{min}}{\ell_{max}-\ell_{min}},0,1\right)
\cdot
\mathrm{clip}\left(\frac{\log(1+\text{volume24hrClob})-v_{min}}{v_{max}-v_{min}},0,1\right)
]

默认门槛：

* `acceptingOrders = true`
* `enableOrderBook = true`
* `liquidityClob >= 50_000 USDC`
* `volume24hrClob >= 20_000 USDC`

这些字段都在市场元数据里。([Polymarket 文档][7])

---

## 6. 把目标仓位变成“最优可执行仓位”

这一步决定你不是追价器，而是真正的策略。

## 6.1 book walk 成本函数

Polymarket taker fee 官方公式是：

[
fee = C \times feeRate \times p \times (1-p)
]

并且：

* BUY 的 fee 以 **shares** 收
* SELL 的 fee 以 **USDC** 收
* SDK 会自动处理下单里的 feeRate，但你自己的 sizing/cost engine 仍要把它算进去。([Polymarket 文档][9])

### BUY walk

给定预算 (B) USDC，沿 asks 从低到高吃：

在价格 (p_j)、可吃 shares (q_j) 的一层上：

* 可花的 gross 金额：
  [
  g_j = \min(B_{rem}, p_j \cdot q_j)
  ]

* gross shares：
  [
  c_j = g_j / p_j
  ]

* fee 的 USDC 值：
  [
  f^{usdc}_j = c_j \cdot \theta \cdot p_j(1-p_j)
  ]

* 因为 BUY fee 按 shares 收，所以 fee shares：
  [
  f^{shares}_j = f^{usdc}_j / p_j = c_j \cdot \theta \cdot (1-p_j)
  ]

* net shares：
  [
  c_j^{net}=c_j-f^{shares}_j
  ]

累计后得到：

* `spent_usdc`
* `net_shares`
* `effective_price = spent_usdc / net_shares`

### SELL walk

给定要卖的 shares (Q)，沿 bids 从高到低吃：

在价格 (p_j) 一层上：

[
c_j = \min(Q_{rem}, q_j)
]

[
gross^{usdc}_j = c_j \cdot p_j
]

[
fee^{usdc}_j = c_j \cdot \theta \cdot p_j(1-p_j)
]

[
net^{usdc}_j = gross^{usdc}_j - fee^{usdc}_j
]

累计后得到：

* `sold_shares`
* `recv_usdc`
* `effective_price = recv_usdc / sold_shares`

## 6.2 两个必须满足的门槛

### 盘口滑点

[
slip_{bps} =
10^4\cdot
\frac{|p_{eff}-p_{best}|}{p_{best}}
]

### leader copy gap

[
gap_{bps} =
10^4\cdot
\frac{|p_{eff}-p_{leader}|}{p_{leader}}
]

其中：

* `p_eff`：你按真实 book walk 得到的有效价
* `p_best`：当前 best bid/ask
* `p_leader`：leader activity 里的成交价

**只有当这两个都在阈值内，仓位才有效。**

默认：

* `max_slip_bps = 60`
* `max_gap_bps = 80`

如果 leader 已经过去 30–60 秒，可以把 `max_gap_bps` 再缩紧。

## 6.3 可执行上限

于是每个 asset 最终可执行上限是：

[
N_a^{liq} = \max {N \ge 0: slip_{bps}(N)\le S_{max}, \ gap_{bps}(N)\le G_{max}}
]

最终目标从原始值投影成：

[
\hat q_a = sign(\hat q_a^{raw})\cdot
\min\Big(
|\hat q_a^{raw}|,
N_a^{liq},
N_a^{mkt},
N_a^{event},
N_a^{cash}
\Big)
]

---

## 7. 风险投影器

## 7.1 约束

### 单市场上限

[
|\hat q_a| \le L_{mkt}
]

建议：
[
L_{mkt}=0.02 \cdot E_t
]

### 单事件上限

[
\sum_{a\in event(e)} |\hat q_a| \le L_{event}
]

建议：
[
L_{event}=0.06 \cdot E_t
]

### 单 leader 贡献上限

任何一个 leader 对某个 asset 的目标贡献不得超过该 asset 最终目标的 50%。

### 尾盘约束

* `<24h` 不开新仓
* `24h~72h` 目标砍半
* 只允许减仓

### negRisk 约束

v1 最稳的实现是：

* `negRisk = true` 的市场：**默认禁止新开仓，只允许减仓/平仓**
* 等你把普通 binary 跑顺，再开 negRisk

因为 negRisk 的事件级相互关系更复杂，而市场元数据里也明确把 `negRisk` 标出来了。([Polymarket 文档][7])

## 7.2 投影算法

不要上通用优化器。
低延时版本直接做 **deterministic projection**：

1. 先算所有 asset 的 `raw target`
2. 单 market clamp
3. 单 event clamp
4. 现金 clamp
5. 尾盘 clamp
6. negRisk clamp
7. 过滤小单

过滤小单建议：

* `< 20 USDC` 不下
* `< 2 * min_order_size` 不下

---

## 8. 两阶段执行

这是兼顾低延时和准确率的核心。

## 阶段 A：fast tranche

当 `TRADE` 到达时，先按 provisional delta 打一小段：

[
q^{fast}_a = \rho \cdot \hat q_a^{raw}
]

其中：

* `ρ = 0.25 ~ 0.35`

但仅在以下条件都满足时执行：

* activity age `< 3000ms`
* book age `< 1000ms`
* `gap_bps <= max_gap_fast`
* `slip_bps <= max_slip_fast`
* 该 leader 已有 warm baseline（positions/value 至少成功同步过一次）

## 阶段 B：confirm tranche

立刻把这个 leader 标 dirty，拉 `/positions + /value`。
snapshot 回来后重算最终目标：

[
q^{final}_a
]

再下：

[
\Delta q^{conf}_a = q^{final}_a - q^{own}_a
]

规则：

* 若 `sign(q_fast) == sign(q_final)`：补齐差额
* 若方向反了：先撤余单，再减仓，再反手
* snapshot 永远优先

---

## 9. 订单规划

Polymarket 所有订单底层都是 limit order；FOK/FAK 是“立刻打到现有流动性”的 marketable 形式。官方文档还明确写了：BUY market order 的 `amount` 是美元，SELL market order 的 `amount` 是 shares，`price` 是最差接受价格，不是目标成交价。([Polymarket 文档][2])

所以你的 planner 只需要三种动作：

```rust
enum Action {
    OpenLongUsdc { asset: AssetId, spend_usdc: UsdcMicros, worst_price_ppm: PricePpm },
    ReduceLongShares { asset: AssetId, sell_shares: SharesMicros, worst_price_ppm: PricePpm },
    CancelOrder { local_id: u64 },
}
```

### 默认执行策略

* 快速跟单：`FAK`
* 极端严格、要求要么全成要么不做：`FOK`
* 不做 maker 跟单

### 方向切换顺序

如果目标从多头变空头，不要直接“先买 NO 再卖 YES”之类混在一起。
统一顺序：

1. 减少现有反向仓
2. 等 user WS 回 `MATCHED`
3. 释放现金/风险预算
4. 再开同向新仓

这会慢一点，但稳定很多。

---

## 10. 事件流和模块拆分

## 10.1 推荐目录

```text
src/
  main.rs
  types.rs
  config.rs

  data/
    activity.rs
    positions.rs
    value.rs
    markets.rs

  ws/
    market_ws.rs
    user_ws.rs

  engine/
    normalize.rs
    aggregate.rs
    liquidity.rs
    risk.rs
    sizer.rs
    planner.rs

  exec/
    clob.rs
    orders.rs

  state/
    store.rs
    snapshot.rs

  ui/
    screen.rs

  util/
    fixed.rs
    time.rs
    dedup.rs
```

## 10.2 actor 划分

### `activity_poller`

职责：

* 轮询所有 leader `/activity`
* 只保留 `type=TRADE`
* 去重
* 发 `Event::LeaderTrade`

### `reconcile_worker`

职责：

* 消费 dirty leader 队列
* 拉 `/positions?sizeThreshold=0`
* 拉 `/value`
* 发 `Event::LeaderSnapshot`

### `market_ws`

职责：

* 维护订阅 asset 的本地 orderbook
* 增量更新
* 定期 checksum / hash 校验
* 失步时触发 `POST /books` 重同步

### `user_ws`

职责：

* 接收自己的 order/trade 回报
* 更新本地 own position / open order / fill 状态

### `strategy_loop`

职责：

* 单写者
* 消费所有事件
* 更新 in-memory state
* 调用 `sizer -> planner -> executor`

### `screen`

职责：

* 每 250–500ms 重绘终端

---

## 11. 推荐事件定义

```rust
enum Event {
    LeaderTrade(LeaderTradeEvent),
    LeaderSnapshot(LeaderSnapshot),
    ValueSnapshot(LeaderValueSnapshot),
    BookDelta(BookDeltaEvent),
    BookResync(BookView),
    OwnOrder(OwnOrderEvent),
    OwnTrade(OwnTradeEvent),
    Timer(TimerKind),
    Log(String),
}
```

### 关键时序规则

* `strategy_loop` 是唯一写状态的人
* 其他任务只能发事件
* 同一 leader 同一时刻最多一个 reconcile in-flight
* dirty debounce：300ms
* hard resync：60s 一次
* market meta refresh：10min 一次

---

## 12. sizing engine 的 Rust 接口

```rust
pub struct SizingInput<'a> {
    pub now_ms: UnixMs,
    pub equity_usdc: UsdcMicros,
    pub leaders: &'a [LeaderState],
    pub books: &'a std::collections::HashMap<AssetId, BookView>,
    pub metas: &'a std::collections::HashMap<AssetId, MarketMeta>,
    pub own_positions: &'a std::collections::HashMap<AssetId, OwnPosition>,
    pub cfg: &'a StrategyConfig,
}

pub struct SizingOutput {
    pub targets: Vec<Target>,
    pub deltas: Vec<DesiredDelta>,
    pub diagnostics: Diagnostics,
}

pub fn compute_targets(input: &SizingInput) -> SizingOutput;
```

### `DesiredDelta`

```rust
pub struct DesiredDelta {
    pub asset: AssetId,
    pub current_risk_usdc: i64,
    pub target_risk_usdc: i64,
    pub delta_risk_usdc: i64,
    pub confidence_bps: u16,
    pub max_copy_gap_bps: u16,
    pub max_slip_bps: u16,
    pub tte_bucket: TteBucket,
}
```

---

## 13. `compute_targets()` 伪代码

```rust
fn compute_targets(input: &SizingInput) -> SizingOutput {
    let mut provisional = collect_leader_exposures(input.leaders, input.now_ms, input.cfg);

    // 1) 聚合 leader
    let mut raw_targets = aggregate_exposures(
        &provisional,
        input.equity_usdc,
        input.cfg,
        input.metas,
    );

    // 2) 应用市场状态 gating
    for t in raw_targets.iter_mut() {
        let meta = &input.metas[&t.asset];
        if !meta.accepting_orders || !meta.enable_order_book {
            t.signed_target_risk_usdc = 0;
            t.stale = true;
            continue;
        }

        let tte = meta.end_ts_ms - input.now_ms;
        if tte < input.cfg.no_new_position_before_ms && t.signed_target_risk_usdc.abs() > 0 {
            // 只允许往 0 方向移动
            t.signed_target_risk_usdc = clamp_toward_flat(
                t.signed_target_risk_usdc,
                current_risk(&input.own_positions, &t.asset),
            );
        }

        if meta.neg_risk && input.cfg.block_neg_risk_entries {
            t.signed_target_risk_usdc = clamp_toward_flat(
                t.signed_target_risk_usdc,
                current_risk(&input.own_positions, &t.asset),
            );
        }
    }

    // 3) 单市场/单事件/现金投影
    let projected = project_constraints(
        raw_targets,
        input.own_positions,
        input.metas,
        input.equity_usdc,
        input.cfg,
    );

    // 4) 流动性投影
    let liquid = project_liquidity(
        projected,
        input.books,
        provisional.last_leader_prices(),
        input.cfg,
    );

    // 5) 生成 delta
    let deltas = build_deltas(liquid, input.own_positions, input.cfg);

    SizingOutput {
        targets: liquid,
        deltas,
        diagnostics: collect_diagnostics(...),
    }
}
```

---

## 14. 流动性投影器 `project_liquidity()`

```rust
fn project_liquidity(
    targets: Vec<Target>,
    books: &HashMap<AssetId, BookView>,
    leader_prices: &HashMap<AssetId, PricePpm>,
    cfg: &StrategyConfig,
) -> Vec<Target> {
    let mut out = Vec::with_capacity(targets.len());

    for mut t in targets {
        let book = match books.get(&t.asset) {
            Some(b) => b,
            None => {
                t.signed_target_risk_usdc = 0;
                t.stale = true;
                out.push(t);
                continue;
            }
        };

        let leader_px = leader_prices.get(&t.asset).copied();
        let max_exec = if t.signed_target_risk_usdc > 0 {
            max_buyable_risk_usdc(book, leader_px, cfg)
        } else {
            max_sellable_risk_usdc(book, leader_px, cfg)
        };

        let capped = t.signed_target_risk_usdc
            .abs()
            .min(max_exec.abs());

        t.signed_target_risk_usdc = capped * t.signed_target_risk_usdc.signum();
        out.push(t);
    }

    out
}
```

### `max_buyable_risk_usdc()`

内部用二分搜索：

* 左边 `lo = 0`
* 右边 `hi = per_market_cap`
* 对每个中点 `mid`

  * walk asks
  * 算 `effective_price`
  * 算 `slip_bps`
  * 算 `copy_gap_bps`
  * 满足则 `lo = mid`，否则 `hi = mid - tick`

这样可以在几十微秒到几百微秒内完成，远比通用求解器轻。

---

## 15. 执行器接口

```rust
pub trait ExecutionClient {
    async fn buy_fak_usdc(
        &self,
        asset: &AssetId,
        spend_usdc: UsdcMicros,
        worst_price_ppm: PricePpm,
    ) -> anyhow::Result<LocalOrderId>;

    async fn sell_fak_shares(
        &self,
        asset: &AssetId,
        sell_shares: SharesMicros,
        worst_price_ppm: PricePpm,
    ) -> anyhow::Result<LocalOrderId>;

    async fn cancel(&self, order: LocalOrderId) -> anyhow::Result<()>;
}
```

### 为什么 planner 和 executor 分开

因为 sizing/risk 是纯函数，越纯越好测。
真正的副作用只有：

* 下单
* 撤单
* 写日志

这样你回测、仿真、线上都能共用一套 sizing 内核。

---

## 16. 终端输出该显示什么

你之前就想要终端态，那我建议不要堆花里胡哨的 TUI，直接 ANSI 重绘。

最少显示这几块：

```text
14:32:10  LIVE  equity=12,430.55  deployed=3,102.21  cash=9,328.34
lag: activity_p50=820ms activity_p95=1650ms  book_age=120ms  errs=0

leaders
  alice   stale=0.8s  ewma=182,340  dirty=no   active=7
  bob     stale=2.1s  ewma= 88,420  dirty=yes  active=4

signals
  trump-yes    target=+152.00  now=+95.00   delta=+57.00  conf=82
  btc-50k-no   target=-80.00   now=  0.00   delta=-80.00  conf=76

book/risk
  trump-yes    best_ask=0.61  eff=0.6123  gap=21bp  slip=18bp  tte=12d
  btc-50k-no   best_ask=0.42  eff=0.4278  gap=66bp  slip=43bp  tte=5d

orders
  #4012 BUY  trump-yes   57.00 usdc   FAK   MATCHED
  #4013 SELL btc-50k-no  181.4 sh     FAK   LIVE

diag
  tracking_err=0.037
  copy_gap_reject=2
  stale_book_reject=0
  tail_reject=1
```

### 关键在线指标

你至少要采这 8 个：

* `activity_to_intent_ms`
* `intent_to_post_ms`
* `post_to_MATCHED_ms`
* `MATCHED_to_CONFIRMED_ms`
* `copy_gap_bps`
* `book_slip_bps`
* `tracking_error`
* `reconcile_miss_rate`

---

## 17. 准确率优先的 10 条硬规则

1. **没 baseline 不开仓**
   一个 leader 至少成功同步过一次 `/positions + /value` 后，才允许 fast tranche。

2. **snapshot 胜过 activity**
   provisional 只是抢时效，快照回来立即覆盖。

3. **只把 `TRADE` 当方向触发**
   `SPLIT / MERGE / CONVERSION / MAKER_REBATE` 只更新状态，不触发复制。`/activity` 的类型官方就是这么分的。([Polymarket 文档][3])

4. **book 旧了不下**

   * `book_age > 1000ms`：不开新仓
   * `book_age > 3000ms`：只允许撤单/减仓

5. **尾盘不开新仓**

   * `<24h`: entry off
   * `<72h`: entry half

6. **negRisk 先禁新开仓**

7. **同 leader 同 asset 300ms debounce**
   防 activity 连续抖动造成多次重复下单

8. **reconcile 有节流**
   同一 leader 最多 1 个 in-flight reconcile

9. **user WS 断了降级**
   user 回报流断开时，立即切到“只减仓，不加仓”

10. **每天至少一次硬对账**
    用自己的 open orders / positions / trades 做全量对账，user WS 和本地状态不一致时以交易所状态为准

---

## 18. 低延时优先的 8 条工程规则

1. **单写者状态机**
   所有共享状态只允许 `strategy_loop` 写

2. **hot path 全内存**
   不在热路径写磁盘

3. **持久化用 append-only**
   JSONL 或 `redb`，后台刷盘

4. **market book 用 WS，REST 只做 resync**
   官方也建议实时 orderbook 用 WS。([Polymarket 文档][10])

5. **book walk 用 smallvec**
   活跃盘口前几档通常够用

6. **用 fixed point，不转 Decimal**
   运算更稳定也更快

7. **数据请求并发受 token bucket 控制**
   避免 Cloudflare throttle

8. **executor 单独队列**
   避免 strategy loop 被网络 I/O 卡住

---

## 19. 启动顺序

### Phase 1: warm-up

* 拉 market metadata
* 建 token/asset 映射
* 对 leader 拉一次 `/value`
* 对 leader 拉一次 `/positions?sizeThreshold=0`
* 订阅相关 asset 的 market WS
* user WS 建连
* 本地 own positions 同步完成

### Phase 2: paper mode

* 只出 intents，不下单
* 跑 1–3 天
* 统计：

  * tracking error
  * copy gap
  * fill ratio
  * reconcile drift

### Phase 3: tiny live

* 总风险预算 5%
* 单市场 0.5%
* fast tranche 20%

### Phase 4: normal live

* 总风险预算 25%
* 单市场 2%
* 单事件 6%
* fast tranche 30%

---

## 20. 默认参数

```toml
[sizing]
global_kappa_bps = 6000
agreement_alpha = 1.5
signal_half_life_ms = 60000
fast_tranche_bps = 3000

[risk]
max_total_deploy_bps = 2500
max_market_bps = 200
max_event_bps = 600
block_neg_risk_entries = true
no_new_position_before_ms = 86400000
half_size_before_ms = 259200000
min_order_usdc = 20000000

[liquidity]
max_slip_bps = 60
max_gap_bps = 80
max_slip_fast_bps = 40
max_gap_fast_bps = 50
min_liquidity_clob_usdc = 50000000000
min_volume_24h_clob_usdc = 20000000000

[poll]
activity_poll_ms = 1500
leader_refresh_ms = 600000
market_meta_refresh_ms = 600000
hard_resync_ms = 60000
dirty_debounce_ms = 300
```

---

## 21. 一句话总结成实现原则

**信号上只信 leader 的“净仓变化”，速度上先用 activity 抢 30%，真相上再用 positions 纠偏；仓位上用 USDC 风险资本做统一单位；执行上只做 orderbook-walk 之后仍然满足 gap/slip/risk 的那部分。**

这套东西真要交给工程同学，就按下面这 5 个函数落地：

```rust
poll_activity()      // 发现 leader 新交易
reconcile_leader()   // 重建 leader 真仓
compute_targets()    // 计算多 leader 聚合目标
project_liquidity()  // 用 book walk 和成本约束裁剪
execute_deltas()     // 下 FAK / 处理回报
```

如果你愿意，我下一条直接把它再压成一份 **Rust trait + struct + main event loop 的最小可跑骨架**。

[1]: https://docs.polymarket.com/cn/api-reference/introduction?utm_source=chatgpt.com "简介 - Polymarket Documentation"
[2]: https://docs.polymarket.com/trading/orders/create "Create Order - Polymarket Documentation"
[3]: https://docs.polymarket.com/api-reference/core/get-user-activity "Get user activity - Polymarket Documentation"
[4]: https://docs.polymarket.com/api-reference/core/get-current-positions-for-a-user "Get current positions for a user - Polymarket Documentation"
[5]: https://docs.polymarket.com/api-reference/wss/market "Market Channel - Polymarket Documentation"
[6]: https://docs.polymarket.com/api-reference/wss/user "User Channel - Polymarket Documentation"
[7]: https://docs.polymarket.com/api-reference/markets/get-market-by-id?utm_source=chatgpt.com "Get market by id - Polymarket Documentation"
[8]: https://docs.polymarket.com/api-reference/rate-limits "Rate Limits - Polymarket Documentation"
[9]: https://docs.polymarket.com/cn/trading/fees?utm_source=chatgpt.com "费用 - Polymarket Documentation"
[10]: https://docs.polymarket.com/trading/orderbook?utm_source=chatgpt.com "Orderbook - Polymarket Documentation"

