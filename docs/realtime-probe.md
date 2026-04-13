# Realtime Probe

This command attempts a shadow-first websocket probe against the markets stream.

Example:

```bash
PYTHONPATH=polymarket_arb python -m polymarket_arb.app.realtime_probe_cli \
  --ws-url wss://api.polymarket.us/v1/ws/markets \
  --market-ids <market-slug> \
  --limit 1 \
  --output ./probe.json
```

If websocket support is unavailable, the command fails closed.
