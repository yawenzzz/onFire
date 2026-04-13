# WebSocket Auth Notes

Polymarket US documentation indicates WebSocket markets authentication is required in the connection handshake.

Current code supports passing these headers when available:
- `X-PM-Access-Key`
- `X-PM-Signature`
- `X-PM-Timestamp`

Important:
- keep this path **shadow-first**
- do not enable trading just because websocket auth succeeds
- authenticated connectivity and trading authorization are separate gates
