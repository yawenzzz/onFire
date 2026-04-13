from __future__ import annotations

import json
import urllib.parse
import urllib.request
import urllib.error


class AccountClient:
    def __init__(self, base_url: str = "https://api.polymarket.us") -> None:
        self.base_url = base_url.rstrip("/")

    def _open_json(self, url: str, auth: dict):
        req = urllib.request.Request(
            url,
            headers={
                "User-Agent": "Mozilla/5.0",
                "Accept": "application/json",
                "X-PM-Access-Key": auth["access_key"],
                "X-PM-Signature": auth["signature"],
                "X-PM-Timestamp": auth["timestamp"],
            },
        )
        with urllib.request.urlopen(req, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))

    def fetch_account_balances(self, auth: dict) -> dict:
        return self._open_json(f"{self.base_url}/v1/account/balances", auth)

    def fetch_open_orders(self, auth: dict) -> dict:
        return self._open_json(f"{self.base_url}/v1/orders/open", auth)

    def fetch_activities(self, auth: dict, limit: int = 10) -> dict:
        query = urllib.parse.urlencode({"limit": limit})
        return self._open_json(f"{self.base_url}/v1/portfolio/activities?{query}", auth)

    def probe_auth_status(self, auth: dict) -> tuple[bool, str]:
        try:
            self.fetch_account_balances(auth)
            return True, "private account api connected"
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace").strip()
            if body:
                return False, body
            return False, f"HTTP {exc.code}"
        except Exception as exc:
            return False, f"{type(exc).__name__}: {exc}"
