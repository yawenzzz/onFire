import unittest

from polymarket_arb.auth.headers import build_auth_headers


class AuthHeadersTests(unittest.TestCase):
    def test_builds_header_names(self) -> None:
        headers = build_auth_headers('key-id', 'sig', '1234567890')
        self.assertEqual(headers['X-PM-Access-Key'], 'key-id')
        self.assertEqual(headers['X-PM-Signature'], 'sig')
        self.assertEqual(headers['X-PM-Timestamp'], '1234567890')
