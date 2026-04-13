# Secret Setup

Use local environment variables only.

## Generate fresh PM auth in your current shell
For Polymarket WebSocket auth, keep the long-lived developer credentials locally and derive fresh `PM_*` values on demand:

```bash
export POLYMARKET_KEY_ID=...
export POLYMARKET_SECRET_KEY=...

source scripts/generate_pm_auth.sh
```

This script:
- reads `POLYMARKET_KEY_ID`
- reads `POLYMARKET_SECRET_KEY`
- derives fresh `PM_ACCESS_KEY`, `PM_TIMESTAMP`, and `PM_SIGNATURE`
- defaults to signing `GET /v1/ws/markets`

Optional path override:

```bash
PM_PATH=/v1/ws/private source scripts/generate_pm_auth.sh
```

Do not store generated `PM_TIMESTAMP` or `PM_SIGNATURE` as long-lived values; they are meant to be used immediately.

## Generate CLOB credentials into `.env.local`
If you already have the wallet inputs locally, you can generate the `CLOB_API_KEY`, `CLOB_SECRET`, and `CLOB_PASS_PHRASE` triplet and write it into `.env.local` with one command:

```bash
export PRIVATE_KEY=...
# optional:
# export SIGNATURE_TYPE=0
# export FUNDER=0x...

bash scripts/generate_clob_env.sh
```

The script:
- calls `scripts/derive_clob_creds_python_template.py`
- creates `.env.local` from `.env.local.example` if needed
- writes the generated `CLOB_*` values into `.env.local`
- verifies the file with `scripts/check_secrets.sh`

It does not write `PRIVATE_KEY` into `.env.local`.

## Run the CLOB account-only view
The account-only monitor path uses CLOB Level 2 auth. In practice that means you need:

- `CLOB_API_KEY`
- `CLOB_SECRET`
- `CLOB_PASS_PHRASE`
- `PRIVATE_KEY` or `CLOB_PRIVATE_KEY`

And per the official CLOB auth docs, many Polymarket accounts also need:

- `SIGNATURE_TYPE`
- `FUNDER_ADDRESS`

Official values:
- `0` = EOA
- `1` = POLY_PROXY
- `2` = GNOSIS_SAFE

The docs say the Polymarket-displayed wallet/proxy address should be used as the funder address.

Without the private key, the monitor will stay fail-closed and show:
- `mode=account-ready`
- `reason=missing private key for clob level2 auth`

Example:

```bash
export PRIVATE_KEY=...
# or:
# export CLOB_PRIVATE_KEY=...
export SIGNATURE_TYPE=0
# or 1 / 2 depending on account type
export FUNDER_ADDRESS=0x...

PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.monitor_cli --root . --account-only
```

## Manual synchronous order draft
The current live trading path is **manual and synchronous**:

- configure one explicit order draft locally
- open the account-only TUI
- press `p`
- the app submits once and immediately refreshes the account snapshot

Required draft variables:

```bash
export ORDER_TOKEN_ID=...
export ORDER_SIDE=BUY
export ORDER_PRICE=0.42
export ORDER_SIZE=5
export ORDER_TYPE=GTC
```

Supported order types currently passed through to the CLOB client:
- `GTC`
- `FOK`
- `FAK`
- `GTD`

Example full command:

```bash
set -a
source .env.local
set +a

PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.monitor_cli --root . --account-only
```

Then inside the TUI:
- `Tab` switches focus
- `j/k` and `PgUp/PgDn` scroll the focused section
- press `p` to submit the configured draft once

If the draft is missing or malformed, the monitor stays fail-closed and shows the error instead of crashing.

## Shell exports
```bash
export PRIVATE_KEY=...
export CLOB_PRIVATE_KEY=...
export SIGNATURE_TYPE=...
export FUNDER_ADDRESS=...
export ORDER_TOKEN_ID=...
export ORDER_SIDE=BUY
export ORDER_PRICE=...
export ORDER_SIZE=...
export ORDER_TYPE=GTC
export POLYMARKET_KEY_ID=...
export POLYMARKET_SECRET_KEY=...
export PM_ACCESS_KEY=...
export PM_SIGNATURE=...
export PM_TIMESTAMP=...
export CLOB_API_KEY=...
export CLOB_SECRET=...
export CLOB_PASS_PHRASE=...
```

## Local file
Create `.env.local`, then load it:
```bash
set -a
source .env.local
set +a
```

do not commit secrets to git.
