# WebSocket Auth Demo

Export credentials before probing:

```bash
export PM_ACCESS_KEY=your-key-id
export PM_SIGNATURE=your-signature
export PM_TIMESTAMP=$(date +%s)
```

Then run the realtime probe or capture path with those values forwarded into the WS auth header builder.
