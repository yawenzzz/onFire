# Live Market Feed Notes

For the current prototype, the recommended live market data surface is the official markets WebSocket endpoint:

- `wss://api.polymarket.us/v1/ws/markets`

Integration rule:
- stay **shadow-first**
- normalize raw messages into `MarketSnapshot`
- do not couple live feed ingestion directly to live order placement
- preserve fail-closed posture when freshness or parsing assumptions break
