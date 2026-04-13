# Realtime Connect Example

Install websocket support:

```bash
pip install websockets
```

Recommended market stream endpoint:

- `wss://api.polymarket.us/v1/ws/markets`

Usage guidance:
- keep the system **shadow-first**
- capture realtime messages to jsonl first
- only then run replay / certification / dashboard generation


For websockets>=15, auth headers are passed via `additional_headers`. Older versions may use `extra_headers`.
