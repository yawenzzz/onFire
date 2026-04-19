可以，下面这版我会按 **“轻量、固定分区、ANSI 重绘、不给终端太大压力”** 来设计。
目标是让 AI 直接照着实现，不需要再猜布局。

## 推荐终端规格

优先支持这两个尺寸：

* **标准模式**：`140 x 38`
* **紧凑模式**：`110 x 32`

刷新频率建议：

* **UI 重绘**：`500ms`
* **数值采样**：`100ms ~ 1000ms` 各模块自己更新
* **日志区**：只保留最近 `8~12` 行

---

# 一、标准布局草图（140 x 38）

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 14:32:10  LIVE  HEALTH=WARN  equity=12,430.55  cash=9,328.34  deployed=3,102.21  gross=3,880.10  net=1,844.20  uptime=02:14:55        │
│ loop_p95=42ms  mon_drop=0  q(mon)=2  q(exec)=1  cpu=11%  rss=82MB  tasks=9  build=dev-4f9a2c                                           │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ FEEDS                                                                                          │ PROCESS                                 │
│ market_ws: UP   age=120ms   pong_p95=210ms  reconnect=1                                        │ main_loop_lag_p95=42ms                  │
│ user_ws  : UP   age=480ms   pong_p95=260ms  reconnect=0                                        │ strategy_q=1  monitor_q=2  exec_q=1    │
│ activity : p50=110ms p95=280ms  err1m=0  429_1m=0                                              │ dropped_mon_events=0                    │
│ positions: p50=190ms p95=910ms  err1m=1  timeout_5m=0                                          │ task_restart_1h=0                       │
│ books    : p50=75ms  p95=160ms  resync_5m=1                                                    │ panic_count=0                           │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ LEADERS (Top 5)                                                                                │ BOOKS (Hot Assets Top 5)                │
│ alice  stale=0.8s  dirty=no   act_p95=820ms  rec_p95=220ms  drift=48bp  pos=7  val=182,340    │ trump-yes    age= 90ms spread=18bp      │
│ bob    stale=8.1s  dirty=yes  act_p95=1.6s   rec_p95=910ms  drift=132bp pos=4  val= 88,420    │ btc-50k-no   age=410ms spread=36bp      │
│ carol  stale=1.3s  dirty=no   act_p95=620ms  rec_p95=180ms  drift=31bp  pos=9  val=214,220    │ eth-q4-yes   age=130ms spread=22bp      │
│ dave   stale=3.5s  dirty=no   act_p95=980ms  rec_p95=330ms  drift=64bp  pos=5  val=104,900    │ sol-70-no    age=220ms spread=44bp      │
│ erin   stale=0.9s  dirty=no   act_p95=710ms  rec_p95=240ms  drift=52bp  pos=6  val=156,030    │ fed-cut-yes  age=180ms spread=27bp      │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ SIGNALS                                                                                        │ EXECUTION                               │
│ trump-yes   raw=+152.0  final=+57.0   now=+95.0   agree=82%  fresh=1.2s  leaders=3             │ a->i p95=190ms  i->post p95=82ms        │
│ btc-50k-no  SKIP gap_too_wide  fresh=2.3s  leader_px=0.4200  eff_px=0.4278                      │ post->match p95=640ms conf_p95=1.8s     │
│ eth-q4-yes  raw= +88.0  final=+40.0   now= +0.0   agree=76%  fresh=0.9s  leaders=2             │ copy_gap p50/p95=14/24bp                │
│ sol-70-no   SKIP stale_book     fresh=0.7s                                                      │ slip     p50/p95=11/18bp                │
│ fed-cut-yes raw= +44.0  final= +0.0   now=+20.0   clip=tail_window                              │ fee_adj  p50/p95=19/31bp                │
│                                                                                                 │ fill_ratio p50/p95=100/100%             │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ RISK                                                                                           │ TRACKING                                │
│ market_top1=152.0  event_top1=214.0  event_top3=522.0                                           │ track_err_now=47bp                      │
│ tail<24h=0.0   tail<72h=214.0   negRisk=0.0                                                     │ rmse_1m=63bp  rmse_5m=71bp              │
│ hhi=1380bp    follow_ratio=84%                                                                  │ eligible=1,420.0 copied=1,193.0         │
│ limits: total=OK market=OK event=WARN cash=OK                                                   │ overcopy=42.0  undercopy=185.0          │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ALERTS                                                                                                                               2/4 │
│ WARN  positions_slow        bob reconcile p95 910ms                                                                                   │
│ WARN  copy_gap_wide         btc-50k-no skipped 4 times / 1m                                                                           │
│ INFO  book_resync           btc-50k-no resynced 1 time / 5m                                                                           │
│ INFO  tail_clip             fed-cut-yes final target clipped to 0                                                                     │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ LOGS                                                                                                                                       │
│ 14:32:08 order#4012 MATCHED trump-yes buy 57.00usdc gap=21bp slip=18bp fee=0.12                                                       │
│ 14:32:09 leader bob dirty -> reconcile                                                                                                  │
│ 14:32:10 book resync btc-50k-no                                                                                                         │
│ 14:32:10 signal eth-q4-yes planned raw=88 final=40                                                                                     │
│ 14:32:10 alert WARN positions_slow bob p95=910ms                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

# 二、分区含义

## 1) Header，固定 2 行

这个区域永远不滚动，给你第一眼判断系统是不是活着。

第一行放全局资金和健康状态：

* 当前时间
* 运行模式：`PAPER / LIVE`
* 总健康灯：`OK / WARN / CRIT`
* `equity`
* `cash`
* `deployed`
* `gross`
* `net`
* `uptime`

第二行放进程健康：

* `loop_p95`
* `mon_drop`
* `q(mon)`
* `q(exec)`
* `cpu`
* `rss`
* `tasks`
* `build`

---

## 2) FEEDS + PROCESS

这里判断“是不是外部慢了，还是你自己卡了”。

### FEEDS 左侧

固定展示 5 行：

* `market_ws`
* `user_ws`
* `activity`
* `positions`
* `books`

每行建议字段：

* 状态：`UP / DOWN / DEGRADED`
* 年龄：`age`
* RTT 或 p95
* reconnect 次数
* 1 分钟错误数
* 429 数

### PROCESS 右侧

固定展示：

* `main_loop_lag_p95`
* `strategy_q`
* `monitor_q`
* `exec_q`
* `dropped_mon_events`
* `task_restart_1h`
* `panic_count`

---

## 3) LEADERS

最多展示 Top 5，永远固定高度，不滚动。

每行结构建议这样：

```text
{name} stale={x}s dirty={yes/no} act_p95={x} rec_p95={x} drift={x}bp pos={n} val={v}
```

字段解释：

* `stale`：离上次成功 snapshot 多久
* `dirty`：是否在等待 reconcile
* `act_p95`：activity 到达延迟
* `rec_p95`：reconcile 延迟
* `drift`：provisional 和 snapshot 差异
* `pos`：当前 positions 数量
* `val`：leader 组合价值

显示规则：

* `dirty=yes` 用黄
* `stale > warn` 用黄
* `drift > warn` 用黄，`> crit` 用红

---

## 4) BOOKS

右侧对应 Hot Assets Top 5。

每行结构：

```text
{asset} age={x}ms spread={y}bp
```

可扩成：

```text
{asset} age=90ms spread=18bp levels=18/22 crossed=no resync=0
```

你真正要看的只有三件事：

* book age
* spread
* 是否 crossed / resync 过多

---

## 5) SIGNALS

这是“策略脑子”面板。

每行只显示最近最活跃的 4~6 个信号。
格式建议分两类。

### 计划执行

```text
trump-yes raw=+152.0 final=+57.0 now=+95.0 agree=82% fresh=1.2s leaders=3
```

### 被跳过

```text
btc-50k-no SKIP gap_too_wide fresh=2.3s leader_px=0.4200 eff_px=0.4278
```

关键字段：

* `raw`
* `final`
* `now`
* `agree`
* `fresh`
* `leaders`
* `reason`

颜色建议：

* `PLANNED` 用绿
* `SKIP` 用黄
* `BLOCKED` 用红

---

## 6) EXECUTION

这个区域只看执行质量。

建议固定展示 6 行：

* `a->i p95`
* `i->post p95`
* `post->match p95`
* `conf p95`
* `copy_gap p50/p95`
* `slip p50/p95`
* `fee_adj p50/p95`
* `fill_ratio p50/p95`

如果空间不够，把最后三项合并成一行：

```text
gap 14/24bp  slip 11/18bp  fee_adj 19/31bp  fill 100/100%
```

---

## 7) RISK

这个区域必须稳，不要滚。

建议 4 行：

```text
market_top1=152.0  event_top1=214.0  event_top3=522.0
tail<24h=0.0  tail<72h=214.0  negRisk=0.0
hhi=1380bp  follow_ratio=84%
limits: total=OK market=OK event=WARN cash=OK
```

重点是让你一眼看到：

* 有没有超单市场
* 有没有超单事件
* 有没有尾盘
* 有没有 negRisk
* 集中度高不高
* 你到底复制了 leader 的多少

---

## 8) TRACKING

专门看“你是不是还在跟着 leader”。

建议字段：

* `track_err_now`
* `rmse_1m`
* `rmse_5m`
* `eligible`
* `copied`
* `overcopy`
* `undercopy`

这是你判断策略是否正在“漂”的核心面板。

---

## 9) ALERTS

固定 4 行最合适，不要更多。
多了终端会吵死。

格式统一：

```text
WARN  positions_slow      bob reconcile p95 910ms
WARN  copy_gap_wide       btc-50k-no skipped 4 times / 1m
INFO  book_resync         btc-50k-no resynced 1 time / 5m
INFO  tail_clip           fed-cut-yes final target clipped to 0
```

建议按优先级排序：

1. `CRIT`
2. `WARN`
3. `INFO`

右上角可以放分页状态：`2/4` 表示共 4 条显示第 2 页，但第一版甚至可以不做分页。

---

## 10) LOGS

底部 5 行到 8 行就够。
只记录最近关键动作，不做 debug dump。

建议只写这些类型：

* order matched / confirmed / rejected
* leader dirty / reconcile start / done
* book resync
* signal planned / skipped
* alert raised / cleared

---

# 三、紧凑版草图（110 x 32）

如果终端比较窄，建议变成上下结构，不要强行左右切很多栏。

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 14:32:10 LIVE WARN eq=12430.55 cash=9328.34 dep=3102.21 gross=3880.10 net=1844.20 up=02:14:55          │
│ lag=42ms mon_drop=0 q=2/1 cpu=11% rss=82MB build=dev-4f9a2c                                               │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ FEEDS  mkt_ws UP 120ms | user_ws UP 480ms | activity p95 280ms | positions p95 910ms | books p95 160ms │
│ PROC   loop_p95 42ms  mon_q 2  exec_q 1  restarts 0  panic 0                                             │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ LEADERS                                                                                                   │
│ alice stale=0.8s drift=48bp pos=7 val=182340 | bob stale=8.1s drift=132bp pos=4 val=88420              │
│ carol stale=1.3s drift=31bp pos=9 val=214220 | dave stale=3.5s drift=64bp pos=5 val=104900             │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ HOT ASSETS                                                                                                │
│ trump-yes age=90ms spread=18bp | btc-50k-no age=410ms spread=36bp | eth-q4-yes age=130ms spread=22bp   │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ SIGNALS                                                                                                   │
│ trump-yes   raw=+152 final=+57 now=+95 agree=82% fresh=1.2s                                              │
│ btc-50k-no  SKIP gap_too_wide fresh=2.3s px=0.4200->0.4278                                               │
│ eth-q4-yes  raw=+88 final=+40 now=0 agree=76% fresh=0.9s                                                 │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ EXEC / RISK                                                                                               │
│ a->i 190ms  i->post 82ms  post->match 640ms  conf 1.8s                                                   │
│ gap 14/24bp  slip 11/18bp  fee_adj 19/31bp  fill 100/100%                                                │
│ tail24=0 tail72=214 neg=0 hhi=1380 follow=84% rmse1m=63bp                                                │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ALERTS                                                                                                    │
│ WARN positions_slow bob p95 910ms                                                                         │
│ WARN copy_gap_wide btc-50k-no skipped 4 times / 1m                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ LOGS                                                                                                      │
│ 14:32:08 MATCHED trump-yes buy 57.00 gap=21bp slip=18bp                                                   │
│ 14:32:09 leader bob dirty -> reconcile                                                                    │
│ 14:32:10 book resync btc-50k-no                                                                           │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

# 四、颜色规范

第一版不要做复杂主题，够用就行。

建议：

* `OK / UP / MATCHED / planned`：绿色
* `WARN / SKIP / dirty / stale`：黄色
* `CRIT / DOWN / rejected / risk breach`：红色
* `INFO / resync / clip / log`：青色
* 标题和边框：灰白

字段高亮规则：

* 数值超过 warn 阈值：黄
* 超过 crit 阈值：红
* 恢复正常后：回到默认色

---

# 五、实现约束，直接给 AI

你可以把下面这段直接丢给实现 AI。

```text
实现一个轻量终端监控面板，要求：
1. 使用 ANSI 清屏重绘，不依赖重型 TUI 框架。
2. 默认布局适配 140x38，自动降级到 110x32 的紧凑布局。
3. 顶部 2 行固定为全局状态栏。
4. 下面按固定分区渲染：FEEDS / PROCESS / LEADERS / BOOKS / SIGNALS / EXECUTION / RISK / TRACKING / ALERTS / LOGS。
5. 每个分区高度固定，不允许内容把布局顶乱。
6. ALERTS 只显示最高优先级的 4 条；LOGS 只显示最近 5~8 条。
7. 数据读取与渲染分离：渲染只读取一个只读快照 UiSnapshot。
8. 主线程不得因渲染阻塞；UI 每 500ms 刷新。
9. 颜色仅使用：绿、黄、红、青、灰白。
10. 若终端宽度不足 110，则自动进入极简模式，只显示 Header + Feeds + Exec + Risk + Alerts。
```

---

# 六、字段名模板

如果你想让 AI 少猜，直接给它这个 `UiSnapshot` 模板。

```rust
struct UiSnapshot {
    now: String,
    mode: String,
    health: String,

    equity: f64,
    cash: f64,
    deployed: f64,
    gross: f64,
    net: f64,
    uptime: String,

    loop_p95_ms: u32,
    mon_drop: u64,
    monitor_q: usize,
    exec_q: usize,
    cpu_pct: f32,
    rss_mb: u32,
    tasks: usize,
    build: String,

    feeds: FeedsView,
    process: ProcessView,
    leaders: Vec<LeaderRow>,
    books: Vec<BookRow>,
    signals: Vec<SignalRow>,
    execution: ExecView,
    risk: RiskView,
    tracking: TrackingView,
    alerts: Vec<AlertRow>,
    logs: Vec<String>,
}
```

---

# 七、最小极简版

如果你最后想更轻，只保留这 6 块：

1. Header
2. Feeds
3. Leaders
4. Signals
5. Execution
6. Risk + Alerts + Logs

但如果终端够大，我还是建议你用我上面那版标准布局。

你下一步最适合做的是，把这份草图和字段模板一起丢给 AI，让它先出一个 **静态 mock 渲染**，确认排版后再接实时数据。

