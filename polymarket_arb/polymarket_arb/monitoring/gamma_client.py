from __future__ import annotations

import json
import time
import urllib.parse
import urllib.request


class GammaMarketClient:
    def __init__(self, base_url: str = "https://gamma-api.polymarket.com", retries: int = 2, retry_sleep_seconds: float = 0.5) -> None:
        self.base_url = base_url.rstrip("/")
        self.retries = retries
        self.retry_sleep_seconds = retry_sleep_seconds

    def _open_json(self, url: str):
        req = urllib.request.Request(
            url,
            headers={
                "User-Agent": "Mozilla/5.0",
                "Accept": "application/json",
            },
        )
        last_error = None
        for attempt in range(self.retries + 1):
            try:
                with urllib.request.urlopen(req, timeout=20) as response:
                    return json.loads(response.read().decode("utf-8"))
            except Exception as exc:
                last_error = exc
                if attempt >= self.retries:
                    raise
                time.sleep(self.retry_sleep_seconds)
        raise last_error

    def fetch_events(self, limit: int = 50, closed: bool | str = False, offset: int = 0) -> list[dict]:
        query = urllib.parse.urlencode({"limit": limit, "closed": str(closed).lower(), "offset": offset})
        return self._open_json(f"{self.base_url}/events?{query}")

    def fetch_book(self, token_id: str) -> dict:
        req = urllib.request.Request(
            f"https://clob.polymarket.com/book?token_id={urllib.parse.quote(str(token_id))}",
            headers={
                "User-Agent": "Mozilla/5.0",
                "Accept": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
