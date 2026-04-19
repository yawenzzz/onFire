最优解不是“看到 leader 一笔成交就机械等比例抄”，而是做**condition 级 target tracking**：`/activity` 只负责触发，`/positions` + `/value` 负责重建 leader 的当前净仓，公开 orderbook / market WebSocket 决定你此刻真实能跟多少，自己的执行状态再用 user WebSocket 回写。Polymarket 现在的公开接口正好支持这条链路：`/activity` 有 `TRADE`、`transactionHash`、`price`、`side`、`usdcSize`，`/positions` 有 `size`、`avgPrice`、`initialValue`、`currentValue`、`endDate`、`negativeRisk`，`/value` 给组合总值；盘口是公开的，market channel 提供实时订单簿和成交流，user channel 给你自己的 `MATCHED → MINED → CONFIRMED` 回报。([Polymarket 文档][1])

## 1. 仓位模型：用“目标权重”而不是“逐笔跟单”

我建议你维护每个 leader 在每个 condition 上的**带符号目标权重**。对 condition (c)，把 YES 记正，NO 记负。在线目标不要直接用 `currentValue` 做主仓位，因为它会随着行情自己飘，导致 leader 没加仓你却被迫追涨；更稳的做法是用 `initialValue` 或 `size \times avgPrice` 表示 leader 当前仍在承担的资本投入，再用 `currentValue` 只做监控和风控。`/positions` 同时给了 `size`、`avgPrice`、`initialValue`、`currentValue`，所以这两套值都能维护。([Polymarket 文档][2])

定义 leader (i) 的风险权重：

[
x_{i,c}(t)=\frac{I^{YES}*{i,c}(t)-I^{NO}*{i,c}(t)}{\tilde V_i(t)+\varepsilon}
]

其中 (I) 用 `initialValue`，(\tilde V_i) 不是瞬时 `/value`，而是它的 EWMA 平滑值：

[
\tilde V_i(t)=\beta V_i(t)+(1-\beta)\tilde V_i(t-1)
]

这样 denominator 不会因为 leader 组合短时波动而让你的目标仓位乱跳。`/value` 直接给总持仓价值，适合做这个平滑。([Polymarket 文档][3])

## 2. 单 leader 的最佳仓位

单 leader 情况下，不要复制绝对金额，复制**权重**。最实用的目标是：

[
N_c^* = E_t \cdot \eta_i \cdot |x_{i,c}| \cdot f_c \cdot g_c \cdot h_c
]

符号取 (sign(x_{i,c}))。其中：

* (E_t)：你的当前净值
* (\eta_i)：这个 leader 的复制系数，离线回测学出来，初始可取 0.2–0.35
* (f_c)：新鲜度因子
* (g_c)：可复制性 / 盘口因子
* (h_c)：临近到期惩罚因子

我建议：

[
f_c = e^{-\Delta t/\tau_f}
]

[
h_c = \mathrm{clip}\left(\frac{tte_c-t_{\min}}{t_{\max}-t_{\min}},0,1\right)
]

其中 (tte_c) 是 market `endDate` 到现在的剩余时间。你前面已经说要避开尾盘，那就直接把 (t_{\min}) 设成 24h，(t_{\max}) 设成 7d；24 小时内新开仓直接不给或极低权重。市场元数据里有 `endDate`、`acceptingOrders`、`enableOrderBook`、`liquidityClob`、`volume24hrClob`，这些都该进 (g_c)。([Polymarket 文档][4])

## 3. 多 leader 的最佳仓位

多个 leader 时，千万别直接把仓位相加。正确做法是先做**去同质化加权**，再聚合。

先给每个 leader 一个在线有效权重：

[
w_i(t)\propto \eta_i \cdot q_i \cdot e^{-lag_i/\tau_i}\cdot \frac{1}{1+\lambda_\rho \bar\rho_i}
]

这里 (q_i) 是你前面那套长期筛选分数，(\bar\rho_i) 是它和其他 leader 的暴露相关性均值。然后对每个 condition 做**鲁棒聚合**，不要用裸平均，建议 winsorize 后的加权均值或 Huber mean：

[
s_c(t)=\mathrm{HuberMean}*i\big(w_i \cdot clip(x*{i,c},-x_{\max},x_{\max})\big)
]

再算一个一致性分数：

[
a_c(t)=\frac{\left|\sum_i w_i \cdot sign(x_{i,c})\right|}{\sum_i w_i \cdot \mathbf{1}*{|x*{i,c}|>\epsilon}}
]

如果几个 leader 同一 condition 一半看 YES、一半看 NO，这个 (a_c) 会自动变小，你的仓位就会被压缩。

最终原始目标仓位：

[
N_c^{raw}=E_t \cdot B_t \cdot |s_c|^\gamma \cdot a_c^\alpha \cdot f_c \cdot g_c \cdot h_c
]

其中 (B_t) 是你当前允许部署的总风险预算，比如 25%–40% 的净值。这样你跟的是**聚合后的净目标**，不是逐笔 tape。([Polymarket 文档][1])

## 4. 最关键的一层：盘口约束和可复制性

真正决定“最佳仓位”的，不是 leader 有多大，而是**你现在能在盘口里真实拿到多少**。Polymarket 的 orderbook 是公开的，既能拉单本，也能批量拉，单次 batch 最多 500 个 token；market WebSocket 适合实时维护 book，orderbook 响应还带 `tick_size`、`min_order_size` 和 `hash`，可以拿来做本地书的重同步校验。官方还明确建议实时数据用 WebSocket，不要靠轮询盘口。([Polymarket 文档][5])

所以 (g_c) 不应该是一个拍脑袋系数，而应该由**真实 book walk** 算出来。对 BUY：

1. 沿 asks 逐层吃深度，得到给定预算下的成交 shares (Q)
2. 对每层按官方费用公式累计 taker fee：

[
fee = C \times feeRate \times p \times (1-p)
]

3. 算有效买入均价 (p_{eff}^{buy}(Q))

SELL 同理沿 bids 走。Polymarket 还提供 per-token 的 `/fee-rate`，返回 `base_fee`（bps）；费用文档给了 taker fee 公式和按分类不同的费率。([Polymarket 文档][6])

然后定义两个约束：

[
slip_{bps}(Q)=10^4\cdot \frac{p_{eff}(Q)-p_{best}}{p_{best}}
]

[
gap_{bps}(Q)=10^4\cdot \frac{p_{eff}(Q)-p_{leader}}{p_{leader}}
]

其中 BUY 用正方向，SELL 反向定义。你要找的是最大 (Q)，满足：

* `slip_bps <= 你的滑点上限`
* `gap_bps <= 你的 leader-copy gap 上限`
* `Q >= min_order_size`

这个最大可执行名义金额，记成 (N_c^{liq})。最后仓位取：

[
N_c^* = sign(s_c)\cdot \min{N_c^{raw},N_c^{liq},N_c^{mkt},N_c^{event},N_c^{cash}}
]

这里 `N_mkt` 是单市场上限，`N_event` 是单事件上限，`N_cash` 是现金上限。若是 `negativeRisk=true` 的市场或同事件多 outcome，事件上限要按**最坏情形损失**来算，而不是简单把名义金额相加。`negativeRisk` 字段在 `/positions` 和 market metadata 里都有。([Polymarket 文档][2])

## 5. 低延时又保准确：两阶段执行

你如果一味等 `/positions` 全量对账，延时会高；如果只看 `/activity` 就满仓冲，又会误判 leader 的再平衡。最稳的做法是**两阶段执行**。

第一阶段是 fast tranche。收到 leader 的 `TRADE` 后，利用 `/activity` 里的 `side`、`price`、`usdcSize` 先做一个**临时 delta**：

[
\Delta \hat x_{i,c}^{fast}\approx \frac{sign \cdot usdcSize}{\tilde V_i}
]

只打目标仓位的 25%–35%，前提是当前 `gap_bps` 很小、市场接受下单、盘口足够深。([Polymarket 文档][1])

第二阶段是 confirm tranche。把这个 leader 标成 dirty，立刻拉 `/positions?sizeThreshold=0` 和 `/value` 重建它的真实净仓；用新 snapshot 替换 provisional state，再算一次 (N_c^*)，补余量、减多余量或取消后续计划。`/positions` 默认 `sizeThreshold=1`，你必须设成 0，否则小仓会漏掉。([Polymarket 文档][2])

这套做法的好处是：你在速度上不至于完全落后，但大部分仓位仍然建立在快照确认之上，准确率高很多。

## 6. 工程设计：Rust 下的轻量高性能架构

最适合你的不是大而全服务，而是一个单二进制、事件驱动的 Rust 进程。

我会拆成 6 个 actor：

1. `activity_poller`
   轮询 leader 的 `/activity`，只看 `TRADE`，按 `transactionHash` 去重。`/activity` 是公开接口，Data API 总限额 1000 req/10s。([Polymarket 文档][1])

2. `reconcile_worker`
   只对 dirty leader 拉 `/positions?sizeThreshold=0` 和 `/value`。`/positions` 限额 150 req/10s，所以必须“脏了才查”，不能粗暴全量高频扫。([Polymarket 文档][2])

3. `market_ws`
   订阅活跃资产的 market WebSocket，维护本地 L2 书；定时用 `/books` 做批量快照重同步。实时事件用 WS，冷启动和容错用 batch `/books`。([Polymarket 文档][7])

4. `sizing_engine`
   纯函数模块，只吃 leader snapshot、market snapshot、own portfolio，输出 `target_usdc_per_condition`。不要在这里做任何网络 I/O。

5. `risk_engine`
   应用单市场、单事件、单 leader、现金、尾盘、negRisk 上限，然后把目标变成可下单 delta。

6. `executor`
   下 FAK 的 marketable limit。官方文档里，所有订单本质上都是 limit order；FOK/FAK 是立即执行型，BUY 的 amount 是美元，SELL 的 amount 是 shares。自己的订单/成交回报走 user WebSocket，并按 `MATCHED → MINED → CONFIRMED` 更新本地仓位。([Polymarket 文档][8])

Rust 实现上，我会用**单写者 actor 模型**，不是多处共享锁：`activity_poller`、`reconcile_worker`、`market_ws` 都把事件发到一个中心 `strategy_loop`，由它单线程更新内存状态并调用 `executor`。这样时序确定、不会出现“leader 快照和盘口快照交叉覆盖”的竞态。热路径完全放内存里，持久化只做 append-only WAL 或 SQLite WAL 后台刷盘。

## 7. 起步参数

给你一组能直接开工的默认值：

* 总风险预算 (B_t)：0.25
* 单 market 上限：净值的 2%–3%
* 单 event 上限：净值的 5%–8%
* 单 leader 贡献上限：总风险预算的 35%–40%
* fast tranche：30%
* `freshness` 半衰期：60 秒
* 最大 `copy_gap`：100 bps 或 2 个 tick，取更宽者
* 最大盘口滑点：50–80 bps
* 尾盘阈值：24h 内不开新仓，72h 内缩半
* 最小有效订单：20–50 USDC

一句话总结：**用 leader 的风险权重做目标，用 `/activity` 触发，用 `/positions` 纠偏，用 orderbook 决定上限，用两阶段执行兼顾速度和准确率。**

下一条我直接把这套模型展开成 Rust 的模块接口、结构体定义和伪代码。

[1]: https://docs.polymarket.com/api-reference/core/get-user-activity "Get user activity - Polymarket Documentation"
[2]: https://docs.polymarket.com/api-reference/core/get-current-positions-for-a-user "Get current positions for a user - Polymarket Documentation"
[3]: https://docs.polymarket.com/api-reference/core/get-total-value-of-a-users-positions "Get total value of a user's positions - Polymarket Documentation"
[4]: https://docs.polymarket.com/api-reference/markets/get-market-by-id "Get market by id - Polymarket Documentation"
[5]: https://docs.polymarket.com/trading/orderbook "Orderbook - Polymarket Documentation"
[6]: https://docs.polymarket.com/trading/fees "Fees - Polymarket Documentation"
[7]: https://docs.polymarket.com/cn/market-data/websocket/overview "概述 - Polymarket Documentation"
[8]: https://docs.polymarket.com/trading/orders/create "Create Order - Polymarket Documentation"

