import unittest

from polymarket_arb.auth.ws_auth import build_ws_auth_headers


class WSAuthConfigTests(unittest.TestCase):
    def test_build_ws_auth_headers_reuses_auth_header_shape(self) -> None:
        headers = build_ws_auth_headers('key-id', 'sig', '123')
        self.assertEqual(headers['X-PM-Access-Key'], 'key-id')
        self.assertEqual(headers['X-PM-Signature'], 'sig')
        self.assertEqual(headers['X-PM-Timestamp'], '123')
