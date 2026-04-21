# Account Monitor

这个仓库当前保留了两类账户监控入口：

1. **账户快照 / 轮询**
2. **user-channel WebSocket 事件流**

都走 Rust 实现，不依赖 Python。

---

## 1. 一次性查看账户信息

```bash
bash scripts/run_rust_show_account_info.sh --json
```

默认会：
- 从仓库根的 `.env` 读取账户凭证
- 调用 Rust bin `run_copytrader_account_monitor`
- 输出完整 JSON

主要字段：
- `account_status.auth_env_source`
- `account_status.signer_address`
- `account_status.effective_funder_address`
- `account_status.signature_type`
- `account_snapshot.balances`
- `account_snapshot.open_orders`
- `account_snapshot.recent_trades`
- `account_snapshot.activities`
- `account_snapshot.cash_history`
- `account_snapshot.closed_positions`
- `account_snapshot.public_value_records`
- `account_snapshot.positions`
- `account_snapshot.pnl_summary`

---

## 2. 持续轮询账户快照

```bash
bash scripts/run_rust_account_monitor.sh
```

默认行为：
- 每 `5s` 刷新一次
- 输出 JSON
- 同时把最新快照写到：
  - `.omx/account-monitor/latest.json`

常用参数：

```bash
# 只跑一轮
bash scripts/run_rust_account_monitor.sh --max-iterations 1

# 改轮询频率
INTERVAL_SECS=2 bash scripts/run_rust_account_monitor.sh

# 自定义输出文件
OUTPUT_PATH=.omx/account-monitor/account.json bash scripts/run_rust_account_monitor.sh
```

---

## 3. WebSocket user-channel 监控

```bash
bash scripts/run_rust_account_user_ws.sh
```

这个脚本会：
- 走 Rust bin `run_copytrader_account_ws`
- 用 CLOB Level 2 凭证连 authenticated user channel
- 输出实时 `order` / `trade` 事件

如果你只想先收一条事件再退出：

```bash
bash scripts/run_rust_account_user_ws.sh --max-events 1
```

典型事件字段：
- `event.type`
- `event.id`
- `event.market`
- `event.asset_id`
- `event.side`
- `event.price`
- `event.size`
- `event.status`
- `event.transaction_hash`

---

## 4. 当前凭证口径

当前 Rust 账户监控默认按下面规则取身份材料：

1. **优先 `.env`**
2. `.env` 不存在时回退 `.env.local`

关键输出字段：
- `auth_env_source=.env|.env.local|process_env_only`
- `signer_address=...`
- `effective_funder_address=...`
- `signature_type=...`

对于 Magic Link / Proxy 账户：
- `signature_type=1`
- `effective_funder_address` 会推导出实际 proxy wallet

---

## 5. 已知限制

### 轮询版
- 能稳定给出余额 / 订单 / 成交 / 持仓 / pnl 摘要
- 不是事件流

### WebSocket 版
- 更接近实时
- 但是否有事件，取决于账户当前是否真的发生了 order / trade 更新
- 如果账户静默，脚本会持续等待事件

---

## 6. 推荐用法

### 看当前账户状态
```bash
bash scripts/run_rust_show_account_info.sh --json
```

### 做持续轮询监控
```bash
bash scripts/run_rust_account_monitor.sh
```

### 盯实时订单/成交事件
```bash
bash scripts/run_rust_account_user_ws.sh
```
