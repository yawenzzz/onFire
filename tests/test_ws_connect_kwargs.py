import unittest

from polymarket_arb.venue.ws_connect_kwargs import build_ws_connect_kwargs


class WSConnectKwargsTests(unittest.TestCase):
    def test_uses_additional_headers_for_newer_websockets(self) -> None:
        kwargs = build_ws_connect_kwargs(
            headers={'X-Test': '1'},
            connect_signature='(uri, *, additional_headers=None, **kwargs)',
        )
        self.assertEqual(kwargs['additional_headers']['X-Test'], '1')

    def test_falls_back_to_extra_headers_for_older_signatures(self) -> None:
        kwargs = build_ws_connect_kwargs(
            headers={'X-Test': '1'},
            connect_signature='(uri, *, extra_headers=None, **kwargs)',
        )
        self.assertEqual(kwargs['extra_headers']['X-Test'], '1')

    def test_disables_proxy_when_supported(self) -> None:
        kwargs = build_ws_connect_kwargs(
            headers=None,
            connect_signature='(uri, *, additional_headers=None, proxy=True, **kwargs)',
        )
        self.assertIsNone(kwargs['proxy'])
