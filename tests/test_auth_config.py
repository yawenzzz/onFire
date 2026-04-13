import unittest

from polymarket_arb.auth.config import WsAuthConfig


class AuthConfigTests(unittest.TestCase):
    def test_is_complete_only_when_all_fields_present(self) -> None:
        self.assertFalse(WsAuthConfig(access_key='a').is_complete())
        self.assertTrue(WsAuthConfig(access_key='a', signature='b', timestamp='1').is_complete())
