有，但我会先纠正一个方向：**最稳的长期跟踪对象，不是榜一，也不是最高成交量的钱包，而是“同一类别里，周/月都持续赚钱、持有期够长、不是做市/尾盘/结构套利”的中频专才。** 你手上的 leaderboard 只能负责“找人”；真正筛人，要把 leaderboard 和 `/activity`、`/positions`、`/value`、`/traded`，再加市场元数据里的 `endDate`、`acceptingOrders`、`enableOrderBook`、`liquidityClob`、`volume24hrClob` 拼起来。官方接口确实提供了这些字段：leaderboard 有 `proxyWallet / pnl / vol` 和 `category / timePeriod / orderBy`；`/activity` 有 `TRADE`、`MAKER_REBATE` 等类型以及 `timestamp / usdcSize / side / conditionId / slug`；`/positions` 有 `currentValue / avgPrice / endDate / negativeRisk`；`/value` 给总持仓价值，`/traded` 给累计交易市场数；市场对象里有 `endDate / enableOrderBook / liquidityClob / volume24hrClob / acceptingOrders`。([Polymarket 文档][1])

**我会用 90 天滚动窗口，按下面这套“硬过滤 + 打分”来做。**

### 1. 候选池怎么取

不要从 `DAY` 榜单挑人，也不要用 `VOL` 榜单直接选人。
我的做法是：**按类别**分别拉 `WEEK + PNL` 和 `MONTH + PNL`，只保留“在同一类别里，两张榜都出现”的地址；`ALL + PNL` 只当加分项。`VOL` 榜单只拿来当红旗，因为很多高量地址本来就更像做市、套利或高周转流量型账户，而不是适合你复制的方向型交易员。官方 leaderboard 就支持按 `category`、`timePeriod`、`orderBy` 这样取数。([Polymarket 文档][1])

我建议你优先从**单一类别专才**里选，而不是 `OVERALL` 总榜。对自动跟单来说，复制的是“风格稳定性”，不是“历史总成绩”。

### 2. 先剔除做市商和伪装成交易员的高周转账户

这是第一道硬过滤。

**硬规则 A：90 天内只要出现过 `MAKER_REBATE`，直接剔除。**
官方文档写得很清楚，maker rebate 是给“把流动性挂在书上、并被别人吃掉”的地址的，每天以 USDC 发放；`/activity` 里也明确有 `MAKER_REBATE` 这个活动类型。对你来说，这种账户的盈利模式和你的 taker 跟单模式并不一致。([Polymarket 文档][2])

但这条还不够，因为官方也说明了**有些市场是 fee-free**，比如地缘政治和 world events 市场；这类地方做市也可能拿不到 rebate，所以“没有 rebate”不等于“不是做市”。因此我再加一个**行为过滤**：

* `flip60 > 25%` 就剔除
  这里 `flip60` 指：同一 `asset` 上，买入后 60 分钟内又卖出的已匹配份额，占全部卖出份额的比例。
* `median_hold < 6h` 也剔除。
* 如果 `current_value / month_vol < 1%`，并且同时 `flip60 > 20%`，也按做市嫌疑剔除。
  这里 `current_value` 用 `/value`，`month_vol` 用 `MONTH` leaderboard 里的 `vol`。这不是官方定义，而是我给你的实战启发式：**高流量、低库存、快进快出**，很像流动性提供者或结构套利者，而不是适合被复制的方向型玩家。官方接口本身提供了这些字段。([Polymarket 文档][3])

### 3. 专门过滤“尾盘交易员”

你说要避免尾盘，我建议只统计**净加仓**，不要统计减仓。
也就是说，只看 `BUY`，或者更严格一点，只看“导致净仓位上升的买入”。因为好交易员在临近结算时减仓/止盈很正常，但**临近结算才大举开仓**，对你这种自动跟单者通常是最不可复制的。`/activity` 里有 `side / timestamp / usdcSize / conditionId / slug`，市场对象里有 `endDate`，足够你算这个指标。([Polymarket 文档][2])

我会定义两个指标：

```text
tail24 = 近 24 小时到期内发生的净加仓 USDC / 全部净加仓 USDC
tail72 = 近 72 小时到期内发生的净加仓 USDC / 全部净加仓 USDC
```

我的硬阈值是：

* `tail24 > 10%`：剔除
* `tail72 > 25%`：剔除

更稳的“黄金钱包”标准是：

* `tail24 < 5%`
* `tail72 < 15%`

这套规则会明显减少“靠临门一脚信息优势、你一跟就只能接高价”的钱包。

### 4. 只保留持有期足够长的钱包

跟单系统最怕的是：对方赚钱靠的是你根本跟不上的速度。

所以我会从 90 天 `TRADE` 历史里，用 **FIFO** 去重建每个 `asset` 的持仓 lot：买入入队，卖出按先进先出配对，算出每一批被卖出份额的持有时长。官方 `activity` 和 `trades` 都给了 `asset / side / size / price / timestamp / transactionHash`，足够你做这个近似。`/trades` 还有 `takerOnly` 参数，默认就是 `true`。([Polymarket 文档][2])

我会这样卡：

* `median_hold < 6h`：剔除
* `median_hold >= 24h`：强加分
* `p75_hold >= 72h`：再加分

也就是说，你最后想要的是那种**大部分仓位会拿 1–5 天**的钱包，而不是 10 分钟、30 分钟那种。

### 5. 剔除结构套利 / negRisk 主导的钱包

`/positions` 里直接有 `negativeRisk` 字段。对自动跟单来说，如果一个钱包的大量仓位来自 negRisk 结构、转换、篮子、再平衡，那它的行为很可能不是你想复制的“方向判断”，而是结构定价或转换效率。([Polymarket 文档][4])

所以我会加一条：

```text
neg_risk_share = negativeRisk 仓位 currentValue / 全部 currentValue
```

阈值：

* `neg_risk_share > 20%`：剔除
* `neg_risk_share < 10%`：理想

### 6. 只跟“你真的能成交”的钱包

这一步非常重要。
你不是在评“谁聪明”，你是在评“谁可复制”。

市场对象里有 `acceptingOrders`、`enableOrderBook`、`liquidityClob`、`volume24hrClob`。我会把 leader 最近 90 天的净加仓，按这些市场条件做一遍过滤：**至少 70% 的净加仓 USDC，必须来自可下单、开着 orderbook、而且流动性足够的市场。**([Polymarket 文档][5])

我的默认阈值：

* `acceptingOrders = true`
* `enableOrderBook = true`
* `liquidityClob >= 50,000`
* `volume24hrClob >= 20,000`

然后算：

```text
copyable_ratio = 满足上面条件的净加仓 USDC / 全部净加仓 USDC
```

阈值：

* `copyable_ratio < 70%`：剔除
* `copyable_ratio > 80%`：理想

### 7. 只要“专才”，不要“到处碰”

长期最稳的钱包，通常是**单类别专精**，不是今天政治、明天体育、后天 meme、再后天 negRisk 套餐。

做法很简单：把 90 天内每笔净加仓按市场 `category` 归类，算出最大类别的占比：

```text
category_purity = 最大类别净加仓 USDC / 全部净加仓 USDC
```

我的阈值：

* `category_purity < 60%`：剔除
* `category_purity > 70%`：理想

同时再加两个“稳定性”门槛：

* `unique_markets_90d < 8`：太像一两次大事件梭哈，剔除
* `unique_markets_90d > 40`：太散，像高周转流量型，剔除

`/traded` 还能给你全历史交易市场数，我会要求 `traded >= 20` 作为最低 track record。([Polymarket 文档][6])

### 8. 我会怎么打分

在通过上面硬过滤后，再打分。
这是我会长期用的版本：

```text
总分 100 = 25 持续性
         + 20 持有期
         + 15 非尾盘
         + 15 非做市
         + 15 可复制流动性
         + 10 简洁度
```

具体含义：

* **持续性 25**

  * 同类别 `WEEK PNL` 上榜：10
  * 同类别 `MONTH PNL` 上榜：10
  * 同类别 `ALL PNL` 上榜：5

* **持有期 20**

  * `median_hold >= 24h`：10
  * `p75_hold >= 72h`：10

* **非尾盘 15**

  * `tail24` 和 `tail72` 越低越高分

* **非做市 15**

  * `maker_rebate_count = 0`
  * `flip60` 越低越高分
  * `current_value / month_vol` 过低会扣分

* **可复制流动性 15**

  * `copyable_ratio` 越高越高分

* **简洁度 10**

  * `neg_risk_share` 越低越高分
  * 开仓市场数适中加分
  * 过于分散或过于集中都扣分

我不会把 `pnl` 本身权重放太高。原因很简单：你真正要买的是**未来可复制性**，不是历史炫技。

### 9. 给你一个“黄金钱包”模板

如果你问我“最稳的钱包长什么样”，我的答案是：

* 同一类别里，`WEEK PNL` 前 25 且 `MONTH PNL` 前 50
* 90 天内 `MAKER_REBATE = 0`
* `tail24 < 5%`
* `tail72 < 15%`
* `median_hold` 在 **1–5 天**
* `neg_risk_share < 10%`
* `copyable_ratio > 80%`
* `category_purity > 70%`
* `unique_markets_90d` 在 **8–25**
* `traded >= 20`，且当前 `/value` 不是极低

这就是我心里的“能长期跟、又不容易被尾盘和做市污染”的钱包画像。([Polymarket 文档][1])

### 10. 真正落地时，别只跟一个

最稳的做法不是押一个地址，而是：

* **核心池**：3–5 个通过筛选的钱包
* **激活池**：其中当前有仓位、分数最高的 1–2 个
* **执行层**：只复制激活池的净加仓

并且每周重算一次分数。
一旦出现下面任一情况，立刻降级或拉黑：

* 首次出现 `MAKER_REBATE`
* `tail24` 升到 15% 以上
* `median_hold` 掉到 12 小时以下
* `category_purity` 掉到 50% 以下
* `neg_risk_share` 明显抬升

如果你非要只选一个，我会选**同类别周/月持续上榜、尾盘占比最低、持有期最长、copyable_ratio 最高**的那个，而不是 raw PnL 最高的那个。因为对你来说，**能复制** 比 **曾经赚过** 更重要。

[1]: https://docs.polymarket.com/api-reference/core/get-trader-leaderboard-rankings "Get trader leaderboard rankings - Polymarket Documentation"
[2]: https://docs.polymarket.com/api-reference/core/get-user-activity "Get user activity - Polymarket Documentation"
[3]: https://docs.polymarket.com/api-reference/core/get-total-value-of-a-users-positions "Get total value of a user's positions - Polymarket Documentation"
[4]: https://docs.polymarket.com/api-reference/core/get-current-positions-for-a-user "Get current positions for a user - Polymarket Documentation"
[5]: https://docs.polymarket.com/api-reference/markets/get-market-by-id "Get market by id - Polymarket Documentation"
[6]: https://docs.polymarket.com/api-reference/misc/get-total-markets-a-user-has-traded "Get total markets a user has traded - Polymarket Documentation"

