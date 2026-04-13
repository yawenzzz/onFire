from __future__ import annotations

import json
import urllib.parse
import urllib.request


class PublicGatewayClient:
    def __init__(self, base_url: str = 'https://gateway.polymarket.us') -> None:
        self.base_url = base_url.rstrip('/')

    def _open_json(self, url: str):
        req = urllib.request.Request(
            url,
            headers={
                'User-Agent': 'Mozilla/5.0',
                'Accept': 'application/json',
                'Origin': 'https://polymarket.us',
            },
        )
        with urllib.request.urlopen(req, timeout=20) as r:
            return json.loads(r.read().decode('utf-8'))

    def fetch_events(self, limit: int = 1):
        query = urllib.parse.urlencode({'limit': limit})
        return self._open_json(f'{self.base_url}/v1/events?{query}')

    def fetch_market_book(self, slug: str):
        return self._open_json(f'{self.base_url}/v1/markets/{slug}/book')
