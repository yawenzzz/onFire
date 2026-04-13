import unittest

from polymarket_arb.auth.config import WsAuthConfig
from polymarket_arb.auth.validator import validate_ws_auth_config


class AuthValidatorTests(unittest.TestCase):
    def test_reports_missing_fields(self) -> None:
        problems = validate_ws_auth_config(WsAuthConfig(access_key='a'))
        self.assertIn('missing signature', problems)
        self.assertIn('missing timestamp', problems)

    def test_complete_config_has_no_problems(self) -> None:
        problems = validate_ws_auth_config(WsAuthConfig(access_key='a', signature='b', timestamp='1'))
        self.assertEqual(problems, [])
