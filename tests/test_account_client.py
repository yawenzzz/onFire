import unittest
import io
import urllib.error

from polymarket_arb.monitoring.account_client import AccountClient


class StubAccountClient(AccountClient):
    def __init__(self, payload):
        super().__init__()
        self.payload = payload
        self.calls = []

    def _open_json(self, url: str, auth: dict):
        self.calls.append((url, auth))
        return self.payload


class AccountClientTests(unittest.TestCase):
    def test_fetch_account_balances_uses_balances_endpoint(self) -> None:
        client = StubAccountClient({"currentBalance": {"value": "12.34"}})
        auth = {"access_key": "kid", "signature": "sig", "timestamp": "123"}
        payload = client.fetch_account_balances(auth)
        self.assertEqual(payload["currentBalance"]["value"], "12.34")
        self.assertIn("/v1/account/balances", client.calls[0][0])

    def test_fetch_open_orders_uses_open_orders_endpoint(self) -> None:
        client = StubAccountClient({"orders": []})
        auth = {"access_key": "kid", "signature": "sig", "timestamp": "123"}
        payload = client.fetch_open_orders(auth)
        self.assertEqual(payload["orders"], [])
        self.assertIn("/v1/orders/open", client.calls[0][0])

    def test_probe_auth_status_returns_body_for_http_401(self) -> None:
        class FailingClient(AccountClient):
            def _open_json(self, url: str, auth: dict):
                raise urllib.error.HTTPError(
                    url=url,
                    code=401,
                    msg="Unauthorized",
                    hdrs=None,
                    fp=io.BytesIO(b"rpc error: code = Unauthenticated desc = API key not found"),
                )

        ok, reason = FailingClient().probe_auth_status({"access_key": "kid", "signature": "sig", "timestamp": "123"})

        self.assertFalse(ok)
        self.assertIn("API key not found", reason)
