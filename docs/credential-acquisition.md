# Credential Acquisition Guide

## Goal
Explain where the runtime credentials come from and how they relate to the current codebase.

## CLOB credentials
The most important credential triplet for further integration is:
- `CLOB_API_KEY`
- `CLOB_SECRET`
- `CLOB_PASS_PHRASE`

These are typically not hand-invented values. The current best-supported path is to derive or create them through the official CLOB client flow.

Observed client vocabulary:
- Python client exposes `create_or_derive_api_creds()`
- TypeScript client exposes `createOrDeriveApiKey()`

Current implication:
- you generally need a wallet/signing identity first
- then use the official client tooling to create or derive the API credentials
- then export them locally into your shell or `.env.local`

## WebSocket auth fields
The current code also supports:
- `PM_ACCESS_KEY`
- `PM_SIGNATURE`
- `PM_TIMESTAMP`

The long-lived inputs you should store locally are:
- `POLYMARKET_KEY_ID`
- `POLYMARKET_SECRET_KEY`

These map to the current websocket header shape used by the prototype:
- `X-PM-Access-Key`
- `X-PM-Signature`
- `X-PM-Timestamp`

Current repo-native derivation path:
- store `POLYMARKET_KEY_ID` and `POLYMARKET_SECRET_KEY`
- run `source scripts/generate_pm_auth.sh`
- the script derives fresh `PM_ACCESS_KEY`, `PM_SIGNATURE`, and `PM_TIMESTAMP`

Important limitation:
- the exact official handshake/derivation details still need to be validated against the live service behavior
- anonymous handshake currently returns HTTP 401
- therefore this path must remain fail-closed until proven with real credentials

## Safe local setup
Use either direct shell exports or `.env.local`, but never paste credentials into chat and never commit them.

### Shell exports
```bash
export POLYMARKET_KEY_ID=...
export POLYMARKET_SECRET_KEY=...
export PM_ACCESS_KEY=...
export PM_SIGNATURE=...
export PM_TIMESTAMP=...
export CLOB_API_KEY=...
export CLOB_SECRET=...
export CLOB_PASS_PHRASE=...
```

### `.env.local`
```bash
set -a
source .env.local
set +a
```

## Verification
After setting variables, run:
```bash
bash scripts/check_secrets.sh
```
You should see `true` for the credentials you loaded.

## Current boundary
Even with credentials present, live trading is still blocked until venue/product-surface and geographic/compliance eligibility are explicitly resolved.
