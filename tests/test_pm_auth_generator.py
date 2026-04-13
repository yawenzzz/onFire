import base64
import unittest

from polymarket_arb.auth.pm_auth_generator import generate_pm_auth_exports


def _private_key_base64() -> str:
    seed = bytes(range(32))
    public = bytes(reversed(range(32)))
    return base64.b64encode(seed + public).decode("utf-8")


class PMAuthGeneratorTests(unittest.TestCase):
    def test_generates_expected_fields(self) -> None:
        exports = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=_private_key_base64(),
            path="/v1/ws/markets",
            timestamp_ms="1705420800000",
        )

        self.assertEqual(exports["PM_ACCESS_KEY"], "key-123")
        self.assertEqual(exports["PM_TIMESTAMP"], "1705420800000")
        self.assertTrue(exports["PM_SIGNATURE"])

    def test_same_inputs_produce_same_signature(self) -> None:
        first = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=_private_key_base64(),
            path="/v1/ws/markets",
            timestamp_ms="1705420800000",
        )
        second = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=_private_key_base64(),
            path="/v1/ws/markets",
            timestamp_ms="1705420800000",
        )

        self.assertEqual(first["PM_SIGNATURE"], second["PM_SIGNATURE"])

    def test_path_changes_signature(self) -> None:
        markets = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=_private_key_base64(),
            path="/v1/ws/markets",
            timestamp_ms="1705420800000",
        )
        private = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=_private_key_base64(),
            path="/v1/ws/private",
            timestamp_ms="1705420800000",
        )

        self.assertNotEqual(markets["PM_SIGNATURE"], private["PM_SIGNATURE"])

    def test_rejects_invalid_private_key(self) -> None:
        with self.assertRaises(ValueError):
            generate_pm_auth_exports(
                access_key="key-123",
                private_key_base64="not-base64",
                path="/v1/ws/markets",
                timestamp_ms="1705420800000",
            )

    def test_accepts_urlsafe_base64_private_key(self) -> None:
        key = _private_key_base64().replace("+", "-").replace("/", "_").rstrip("=")
        exports = generate_pm_auth_exports(
            access_key="key-123",
            private_key_base64=key,
            path="/v1/ws/markets",
            timestamp_ms="1705420800000",
        )
        self.assertEqual(exports["PM_ACCESS_KEY"], "key-123")
        self.assertTrue(exports["PM_SIGNATURE"])
