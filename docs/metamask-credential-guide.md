# MetaMask Credential Guide

If you log into Polymarket with MetaMask, the practical next step is usually **not** to hand-enter API credentials. Instead, you use your wallet identity to derive or create CLOB credentials via an official client flow.

## Goal
Obtain:
- `CLOB_API_KEY`
- `CLOB_SECRET`
- `CLOB_PASS_PHRASE`

## Practical paths
### Python path
Use the official Python client flow that exposes a credential derivation/create helper.
Typical idea:
1. install the client
2. initialize it with your wallet/signer information
3. call the helper that derives/creates API creds
4. export the returned values locally

### TypeScript path
Use the official TypeScript client flow that exposes API key derivation.
Typical idea:
1. install the client
2. initialize it with your signer/wallet identity
3. call the derivation helper
4. export the returned values locally

## Safety rule
Do not paste your private key into chat.
Do not commit credentials.
Only load them locally through shell exports or `.env.local`.

## After you have the credentials
Export them locally:
```bash
export CLOB_API_KEY=...
export CLOB_SECRET=...
export CLOB_PASS_PHRASE=...
```

Optional websocket fields if available:
```bash
export PM_ACCESS_KEY=...
export PM_SIGNATURE=...
export PM_TIMESTAMP=...
```
