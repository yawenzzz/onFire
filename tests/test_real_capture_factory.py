import unittest

from polymarket_arb.venue.real_capture_factory import build_async_ws_client


class RealCaptureFactoryTests(unittest.TestCase):
    def test_returns_none_without_connect_factory(self) -> None:
        self.assertIsNone(build_async_ws_client('wss://example.test', None))

    def test_returns_client_with_connect_factory(self) -> None:
        client = build_async_ws_client('wss://example.test', lambda url: object())
        self.assertIsNotNone(client)
        self.assertEqual(client.url, 'wss://example.test')
