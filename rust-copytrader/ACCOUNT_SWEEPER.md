# Account Sweeper

这个脚本是一个**独立于主跟单进程**的账户资金回收 loop：

- 定期扫你自己的账户
- 检测能 `merge` 的仓位就 `merge`
- 检测需要 `redeem` 的仓位就 `redeem`
- 目标是把可回收的资金尽快打回 USDC，提高资金利用率

它不会启动主跟单，也不会改主跟单的轮询频率/提交流程。

这个 wrapper 底层调用的 Rust entrypoint 是 `run_copytrader_account_sweeper`。

---

## 1. 直接启动

```bash
bash scripts/run_rust_account_sweeper.sh
```

默认行为：

- 独立 watch loop
- 每 `30s` 扫一轮
- 默认 **live submit**
- 日志持续追加到：

```text
logs/account-sweeper/account-sweeper.log
```

启动时会先打印：

```text
[info]: account sweeper independent loop
[info]: mode=live
[info]: log_file=...
[info]: independent_of_main_follow=true
```

---

## 2. 只看预览，不实际发交易

```bash
ALLOW_LIVE_SUBMIT=0 bash scripts/run_rust_account_sweeper.sh --max-iterations 1
```

这时只会输出 preview 日志，不会真的发链上 `merge` / `redeem`。

---

## 3. 常用参数

### 改扫描频率

```bash
INTERVAL_SECS=60 bash scripts/run_rust_account_sweeper.sh
```

### 只跑固定轮数

```bash
bash scripts/run_rust_account_sweeper.sh --max-iterations 3
```

### 改日志文件

```bash
LOG_FILE=logs/account-sweeper/night.log bash scripts/run_rust_account_sweeper.sh
```

---

## 4. 现在会做什么

### merge

脚本会查询当前账户里 `mergeable=true` 的 positions，按 `condition_id` 聚合：

- 同一个 condition 下同时有 YES / NO
- 就按 `min(yes_size, no_size)` 做 full-set merge

日志示例：

```text
[info]: merge submitted condition_id=0x... shares=5 yes_shares=8 no_shares=5 negative_risk=false slug=... tx_hash=0x... block_number=...
```

### redeem

脚本会查询当前账户里 `redeemable=true` 的 positions，按 `condition_id` 聚合：

- 普通市场：走 `redeem_positions`
- `negative_risk=true`：走 `redeem_neg_risk`

日志示例：

```text
[info]: redeem submitted condition_id=0x... method=redeem_positions yes_shares=12 no_shares=0 negative_risk=false slug=... tx_hash=0x... block_number=...
```

---

## 5. 凭证要求

默认读取仓库根目录的：

- `.env`
- `.env.local`（当 `.env` 不存在时回退）

至少需要：

- `PRIVATE_KEY` / `CLOB_PRIVATE_KEY`
- `POLY_ADDRESS` / `SIGNER_ADDRESS`（如果没配，会尝试从 private key 推导）

如果是 proxy / magic link 账户：

- `SIGNATURE_TYPE`
- `FUNDER_ADDRESS`（或让脚本自动推导 effective funder）

---

## 6. 说明

- 这是**账户回收脚本**，不是跟单脚本
- 它和 `scripts/run_rust_minmax_follow_live_submit.sh` 完全分开跑
- 想降低对主跟单的影响，就把它单独放一个终端 / tmux pane 长挂
