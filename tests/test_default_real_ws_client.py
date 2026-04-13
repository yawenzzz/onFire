import unittest

from polymarket_arb.venue.default_real_ws_client import build_default_ws_client


class DefaultRealWSClientTests(unittest.TestCase):
    def test_returns_none_when_optional_factory_missing(self) -> None:
        client = build_default_ws_client('wss://example.test', optional_factory_resolver=lambda: None)
        self.assertIsNone(client)

    def test_returns_client_when_factory_available(self) -> None:
        client = build_default_ws_client('wss://example.test', optional_factory_resolver=lambda: (lambda url, **kwargs: object()))
        self.assertIsNotNone(client)
        self.assertEqual(client.url, 'wss://example.test')

    def test_preserves_headers_for_runtime_factory(self) -> None:
        seen = {}
        def factory(url, **kwargs):
            seen['kwargs'] = kwargs
            return object()
        client = build_default_ws_client(
            'wss://example.test',
            optional_factory_resolver=lambda: factory,
            headers={'X-Test': '1'},
            connect_signature='(uri, *, additional_headers=None, **kwargs)',
        )
        self.assertIsNotNone(client)
        self.assertIn('additional_headers', seen['kwargs'])
        self.assertEqual(seen['kwargs']['additional_headers']['X-Test'], '1')

    def test_disables_proxy_when_signature_supports_it(self) -> None:
        seen = {}

        def factory(url, **kwargs):
            seen['kwargs'] = kwargs
            return object()

        client = build_default_ws_client(
            'wss://example.test',
            optional_factory_resolver=lambda: factory,
            connect_signature='(uri, *, additional_headers=None, proxy=True, **kwargs)',
        )
        self.assertIsNotNone(client)
        self.assertIn('proxy', seen['kwargs'])
        self.assertIsNone(seen['kwargs']['proxy'])
