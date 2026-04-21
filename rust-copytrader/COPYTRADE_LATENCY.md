# Copytrade Latency & Price Gap

当前仓库已经能输出一条跟单链路里最关键的两类观测：

1. **延时**
2. **leader 价格 vs follower 实际下单价格**

---

## 1. 直接看一份整理好的延时报告

```bash
bash scripts/run_rust_copytrade_latency_report.sh --user <wallet>
```

默认会优先读取：

- `.omx/minmax-follow/<wallet>/latest.txt`

如果你要看 force-live 那条一次性真下单路径的最近一次运行，也可以：

```bash
bash scripts/run_rust_copytrade_latency_report.sh --user <wallet> --source force-live
```

如果你已经知道具体报告文件路径：

```bash
bash scripts/run_rust_copytrade_latency_report.sh --report /path/to/summary.txt
```

JSON 版：

```bash
bash scripts/run_rust_copytrade_latency_report.sh --user <wallet> --json
```

---

## 2. 它会给你什么

### leader
- `leader_timestamp`
- `leader_price`

### watch
- `watch_started_at_unix_ms`
- `watch_finished_at_unix_ms`
- `watch_elapsed_ms`
- `leader_to_watch_finished_ms`

### payload
- `gate_started_at_unix_ms`
- `payload_build_started_at_unix_ms`
- `order_built_at_unix_ms`
- `order_build_elapsed_ms`
- `payload_ready_at_unix_ms`
- `payload_prep_elapsed_ms`
- `gate_queue_delay_ms`
- `capture_to_payload_ready_ms`
- `leader_to_payload_ready_ms`

### submit
- `submit_started_at_unix_ms`
- `submit_finished_at_unix_ms`
- `submit_roundtrip_elapsed_ms`
- `capture_to_submit_started_ms`
- `capture_to_submit_finished_ms`
- `leader_to_submit_started_ms`
- `leader_to_submit_finished_ms`

### pricing
- `follower_effective_price`
- `price_gap`
- `price_gap_bps`
- `adverse_price_gap_bps`

---

## 3. 这些字段从哪里来

### watch 阶段
现在 `run_copytrader_minmax_follow` 已经会输出：
- `watch_started_at_unix_ms`
- `watch_finished_at_unix_ms`
- `watch_elapsed_ms`
- `leader_to_watch_finished_ms`

### gate / payload / submit 阶段
现在 `run_copytrader_live_submit_gate` 已经会输出：
- `gate_started_at_unix_ms`
- `payload_build_started_at_unix_ms`
- `order_built_at_unix_ms`
- `order_build_elapsed_ms`
- `payload_ready_at_unix_ms`
- `payload_prep_elapsed_ms`
- `leader_to_payload_ready_ms`
- `submit_started_at_unix_ms`
- `submit_finished_at_unix_ms`
- `submit_roundtrip_elapsed_ms`
- `leader_to_submit_started_ms`
- `leader_to_submit_finished_ms`

### 价差
`run_copytrader_live_submit_gate` 还会输出：
- `leader_price`
- `follower_effective_price`
- `price_gap`
- `price_gap_bps`
- `adverse_price_gap_bps`

---

## 4. 最推荐的观察方式

### 持续跟单预览链
```bash
bash scripts/run_rust_minmax_follow_live.sh --user <wallet> --loop-count 1
```

这条最适合先看：
- watch 延时
- gate 是否放行
- 当前 leader 基本价格

### 强制真路径
```bash
IGNORE_SEEN_TX=1 bash scripts/run_rust_follow_last_action_force_live_once.sh --user <wallet>
```

这条最适合看：
- payload ready 时延
- submit roundtrip
- follower 实际成交价格
- adverse price gap

---

## 5. 当前已知限制

### 如果 gate 提前 block
比如：
- `activity_source_over_budget`
- `account_snapshot_unreadable`

那你会看到：
- watch 指标有
- payload / submit 指标可能为空

### 最完整的指标
目前依然是：
- `run_rust_follow_last_action_force_live_once.sh`

那条路径最完整。
