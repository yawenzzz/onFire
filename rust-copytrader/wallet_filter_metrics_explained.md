# wallet_filter_v1 指标解释（人话版）

这份文档专门解释 `wallet_filter_v1` 输出里的每个指标是什么意思。

适用对象：
- `discover_copy_leader`
- `scan_copy_leader_categories`
- `.omx/discovery/wallet-filter-v1-*.txt`
- `.omx/discovery/wallet-filter-v1-summary.txt`

---

# 1. 先记住一个总原则

> **硬过滤优先级 > 总分。**

也就是说：
- `score_total` 再高
- 只要 `rejection_reasons` 非空
- 最终还是会被踢掉

所以你看报告时，顺序应该是：

1. `status`
2. `rejection_reasons`
3. 再看 `score_total` 和各个分项

---

# 1.1 这些指标的数据到底从哪来

先说结论：

> **当前这版 `wallet_filter_v1` 不是去 Polygon 链上直接查“谁是做市商”。**

现在用的数据源主要是：

- Polymarket leaderboard
- Polymarket `/activity`
- Polymarket `/positions`
- Polymarket `/value`
- Polymarket `/traded`
- Polymarket / Gamma 的 market metadata

也就是说，当前判断逻辑是：

> **API 行为特征判断**

不是：

> **链上地址取证 / Polygon 原生资金流分析**

所以像下面这些判断：

- `maker_rebate_count`
- `flip60`
- `current_value_to_month_vol`
- `tail24`
- `tail72`

都是根据 **Polymarket 公开接口返回的交易行为** 算出来的，
不是直接扫 Polygon 上的合约交互日志。

### 当前“做市嫌疑”最强信号是什么

最硬的一条就是：

- `/activity` 里出现 `type = MAKER_REBATE`

这在当前实现里会直接转成：

- `maker_rebate_count > 0`
- 然后触发：
  - `maker_rebate_detected`

### 当前还不是做的事情

现在**没有**直接做这些链上增强判断：

- Polygon 上挂单 / 撤单模式分析
- EOA / proxy / funding route 的链上追踪
- LP / 做市机器人的链上调用模式聚类
- 资金流入流出图谱
- 与 Polymarket 合约的更细粒度交互画像

如果以后要做“链上增强版 smart-money 识别”，那会是 **wallet_filter_v2 / operator-enhanced** 方向，不是当前这版 `wallet_filter_v1` 的实现范围。

---

# 2. 一条候选结果怎么读

例子：

```text
wallet=0x492442eab586f242b53bda933fd5de859c8a3782
category=SPORTS
status=rejected
score_total=63
persistence_score=20
hold_score=20
non_tail_score=0
non_maker_score=0
copyable_score=15
simplicity_score=8
week_rank=1
month_rank=1
month_pnl=7127605.119528
month_vol=27970686.499866
vol_red_flag=false
maker_rebate_count=11
flip60=0.000000
median_hold_hours=411.366
p75_hold_hours=508.170
tail24=1.000000
tail72=1.000000
neg_risk_share=0.000000
copyable_ratio=0.773468
category_purity=1.000000
current_value_to_month_vol=0.000000
unique_markets_90d=242
traded_markets=1550
latest_activity_timestamp=1776303488
rejection_reasons=maker_rebate_detected,tail24_above_10pct,tail72_above_25pct,unique_markets_above_40
```

这条结果用一句话总结：

> **它很赚钱，但不适合按 wallet_filter_v1 直接跟。**

原因不是它不强，而是：
- 有做市痕迹
- 尾盘太重
- 近 90 天涉及市场过多，太分散

---

# 3. 基础字段

## `wallet`
候选钱包地址。

## `category`
这个钱包是从哪个类别池里筛出来的。

比如：
- `SPORTS`
- `POLITICS`
- `CRYPTO`
- `WEATHER`

注意：
> 这是“按类别筛”，不是 OVERALL 总榜乱抓。

## `status`
最终结论：
- `passed` = 通过
- `rejected` = 淘汰
- `selected` = 在通过的钱包里又被选中为最终 leader

如果是 `rejected`，就要立刻去看：
- `rejection_reasons`

---

# 4. 总分和分项分数

## `score_total`
总分。

它由这些分项组成：
- `persistence_score`
- `hold_score`
- `non_tail_score`
- `non_maker_score`
- `copyable_score`
- `simplicity_score`

这个分数的作用是：
> **给通过硬过滤的钱包排序。**

不是最终生杀权。

---

## `persistence_score`
持续性分数。

看的是这个钱包是不是在同一类别里：
- `WEEK + PNL` 上榜
- `MONTH + PNL` 上榜
- `ALL + PNL` 上榜

直觉理解：
> 不是一天爆发，而是持续赚钱。

分越高，说明它越像“持续型”而不是“偶发型”。

---

## `hold_score`
持有期分数。

主要看：
- `median_hold_hours`
- `p75_hold_hours`

当前策略更喜欢：
- 中位持有期 >= 24h
- 75 分位持有期 >= 72h

直觉理解：
> 这个钱包是不是愿意拿仓，而不是快进快出。

---

## `non_tail_score`
非尾盘分数。

主要看：
- `tail24`
- `tail72`

分越高，说明：
> 它越不像“结算前才冲进去”的尾盘选手。

如果这一项是 0，通常说明尾盘占比很高。

---

## `non_maker_score`
非做市分数。

主要看：
- `maker_rebate_count`
- `flip60`
- `current_value_to_month_vol`

分越高，说明：
> 它越像方向型交易员，而不是做市 / 高频 / 结构流量号。

如果这一项很低，重点排查：
- 有没有 `maker_rebate`
- 有没有低库存高周转

---

## `copyable_score`
可复制性分数。

看的是它做的市场里，有多少是真正适合你复制的。

核心来源是：
- 市场是否仍可下
- 是否开了 order book
- 流动性 / volume 是否达到阈值

直觉理解：
> 它赚钱的地方，你到底跟不跟得进去。

---

## `simplicity_score`
策略简洁度分数。

主要看：
- `neg_risk_share`
- `category_purity`
- `unique_markets_90d`

直觉理解：
> 它是不是一个容易理解、容易复制、不是结构太脏的玩家。

---

# 5. 榜单相关指标

## `week_rank`
这个钱包在：
- 同类别
- `WEEK + PNL`
榜单里的名次。

数字越小越强。

## `month_rank`
这个钱包在：
- 同类别
- `MONTH + PNL`
榜单里的名次。

数字越小越强。

## `all_rank`
如果报告里有这个字段，表示它在：
- 同类别
- `ALL + PNL`
榜单里的名次。

这个更多是加分项，不是硬门槛。

## `month_pnl`
月度收益。

直觉理解：
> 这个钱包最近一个月账面上赚了多少。

注意：
> 这不是唯一标准。很赚钱，不代表适合跟。

## `month_vol`
月度成交量 / 交易额。

直觉理解：
> 这个钱包最近一个月交易得有多猛。

单独看这个值没有意义，要和其他指标一起看。

---

## `vol_red_flag`
成交量红旗。

- `true` = 这个钱包在相应的高成交量榜里也很突出
- `false` = 没有触发这个红旗

它不是直接判死刑，但会提醒你：
> 这个钱包可能更像做市 / 高周转 / 流量号。

---

# 6. 做市 / 高频 / 翻单行为

## `maker_rebate_count`
在回看窗口内出现 `MAKER_REBATE` 的次数。

直觉理解：
> 有多少次明确出现了做市返佣痕迹。

如果这个值 > 0，通常是大红旗。

因为这意味着：
> 这个钱包的盈利方式，至少部分不是你这种 taker 跟单能复制的。

---

## `flip60`
60 分钟内翻单比例。

粗暴理解：
> 买入后 60 分钟内又卖掉的比例有多高。

- 越高 = 越像快手盘
- 越低 = 越像持仓型

如果：
- `flip60` 很高
- `current_value_to_month_vol` 又很低

通常很像：
- 做市
- 结构套利
- 高频周转

---

# 7. 持有期指标

## `median_hold_hours`
中位持有时长，单位小时。

直觉理解：
> 这钱包最典型的一笔仓，大概拿多久。

例子：
- `24` = 大概 1 天
- `72` = 大概 3 天
- `400+` = 十几天

## `p75_hold_hours`
75 分位持有时长，单位小时。

直觉理解：
> 这个钱包有相当一部分仓位，会拿到多久。

如果它高，说明：
> 不是只会短平快，它真的会拿住仓位。

---

# 8. 尾盘风险指标

## `tail24`
离市场结束 24 小时内的净加仓占比。

取值范围通常是：
- `0.0` 到 `1.0`

例子：
- `0.05` = 只有 5% 的净买入发生在结算前 24 小时
- `1.0` = 100% 都是结算前 24 小时才买

直觉理解：
> 越高，越像尾盘选手。

---

## `tail72`
离市场结束 72 小时内的净加仓占比。

和 `tail24` 一样，只是窗口放大到 72 小时。

如果：
- `tail24` 和 `tail72` 都很高

说明：
> 它的策略明显偏向临近结算阶段。

对于自动跟单，这种通常很危险。

---

# 9. 结构与可复制性指标

## `neg_risk_share`
当前仓位里，负风险 / 结构仓位占比。

- 越低越干净
- 越高越像结构套利 / 组合转换 / 特殊玩法

如果很高，说明：
> 它赚钱方式可能不是普通方向性判断，不适合直接复制。

---

## `copyable_ratio`
可复制市场占比。

大致表示：
> 它的净加仓里，有多少比例发生在你当前模型认为“可复制”的市场上。

越高越好。

如果低，说明：
> 它很多操作发生在你不好复制的市场里。

比如：
- 流动性不够
- 下单窗口不好
- order book 条件不理想

---

## `category_purity`
类别纯度。

直觉理解：
> 这个钱包是不是基本只做一个类别。

- `1.0` = 几乎全在一个类别
- `0.7` = 大部分在一个类别
- 越低 = 越分散

你当前策略偏好的是：
> 单类别专才，而不是到处乱打的人。

---

## `current_value_to_month_vol`
当前总仓位价值 / 月成交量。

直觉理解：
> 它当前留在手里的库存，相对最近月成交量来说重不重。

如果这个值非常低，说明：
> 月成交很多，但库存很轻。

这常常是：
- 高周转
- 做市
- 流量型玩家

单独看这个值不够，要和 `flip60`、`maker_rebate_count` 一起看。

---

# 10. 分散度指标

## `unique_markets_90d`
过去 90 天涉及的不同市场数量。

直觉理解：
> 这钱包最近 90 天到底碰了多少个市场。

- 太少：可能只是押中了几个事件
- 太多：可能太散，不像专才

你当前这套规则里，过大是红旗。

因为：
> 市场数太多，往往意味着风格太分散，不好复制。

---

## `traded_markets`
历史累计交易过的市场总数。

这个更多代表：
> 它是不是老玩家，有没有长期轨迹。

不是硬淘汰条件，但可以辅助判断钱包成熟度。

---

# 11. 时间字段

## `latest_activity_timestamp`
最近一笔 activity 的时间戳（Unix 秒）。

它的作用主要是：
> 告诉你这个钱包最近还有没有在动。

如果你想把它转成人类可读时间，后面可以再加脚本或输出格式化。

---

# 12. 最重要字段：`rejection_reasons`

这个字段决定生死。

只要这里不是空，`status` 基本就是 `rejected`。

下面逐项翻译。

---

## `maker_rebate_detected`
含义：
> 发现了做市返佣记录。

人话：
> 这钱包有明确做市痕迹。

---

## `tail24_above_10pct`
含义：
> `tail24 > 10%`

人话：
> 它有太多买入发生在结算前 24 小时内。

---

## `tail72_above_25pct`
含义：
> `tail72 > 25%`

人话：
> 它有太多买入发生在结算前 72 小时内。

---

## `copyable_ratio_below_70pct`
含义：
> 可复制市场占比低于 70%。

人话：
> 它赚钱的很多地方，你其实不好复制。

---

## `category_purity_below_60pct`
含义：
> 类别纯度低于 60%。

人话：
> 它不是专注单一类别，而是分散得太厉害。

---

## `unique_markets_above_40`
含义：
> 最近 90 天做过的市场数量 > 40。

人话：
> 它碰的市场太多了，太散，不像可稳定复制的专才。

---

## `low_inventory_high_turnover`
含义：
> 当前库存太低，但周转太高。

人话：
> 像流量号 / 做市 / 高频，不像你想跟的方向型钱包。

---

## `neg_risk_share_above_20pct`
含义：
> 结构仓位占比太高。

人话：
> 它靠结构玩法赚的钱太多，不够“纯方向”。

---

## `traded_markets_below_20`
含义：
> 历史交易市场数太少。

人话：
> 样本太小，还不能当成稳定 smart money。

---

## `unique_markets_below_8`
含义：
> 近 90 天市场数太少。

人话：
> 可能只是押中了几次，并不稳定。

---

# 13. 你以后看报告，最推荐顺序

## 第一步：看最终状态
- `status`

## 第二步：看生死原因
- `rejection_reasons`

## 第三步：看它本来强不强
- `week_rank`
- `month_rank`
- `month_pnl`
- `score_total`

## 第四步：看为什么不适合复制
重点看：
- `maker_rebate_count`
- `tail24`
- `tail72`
- `copyable_ratio`
- `category_purity`
- `unique_markets_90d`

---

# 14. 一句话总结这些指标各自回答的问题

- `week_rank / month_rank / month_pnl`：**它赚不赚钱？**
- `median_hold_hours / p75_hold_hours`：**它拿不拿得住？**
- `tail24 / tail72`：**它是不是尾盘选手？**
- `maker_rebate_count / flip60`：**它是不是做市 / 快手盘？**
- `copyable_ratio`：**它的收益你到底跟不跟得进去？**
- `category_purity / unique_markets_90d`：**它是不是专才，还是太散？**
- `neg_risk_share`：**它是不是靠结构玩法赚钱？**
- `rejection_reasons`：**为什么这钱包不能直接跟？**

---

# 15. 对你给的这条钱包，最终一句话解释

这条：
- **很强**
- **很赚钱**
- **拿仓也很稳**

但是它同时：
- 有做市痕迹
- 强烈偏尾盘
- 最近 90 天覆盖市场过多

所以在 `wallet_filter_v1` 里，它的结论就是：

> **值得研究，但不值得直接复制。**
