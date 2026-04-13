# Realtime Ingestion

Recommended market stream endpoint:

- `wss://api.polymarket.us/v1/ws/markets`

Guidelines:
- keep the system **shadow-first**
- capture raw messages to **capture jsonl** before replay/certification
- do not couple realtime ingestion directly to live order placement
