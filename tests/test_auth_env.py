import os
import unittest

from polymarket_arb.auth.env import load_ws_auth_from_env


class AuthEnvTests(unittest.TestCase):
    def test_returns_none_when_missing_env(self) -> None:
        for key in ['PM_ACCESS_KEY', 'PM_SIGNATURE', 'PM_TIMESTAMP']:
            os.environ.pop(key, None)
        self.assertIsNone(load_ws_auth_from_env())

    def test_loads_env_triplet(self) -> None:
        os.environ['PM_ACCESS_KEY'] = 'kid'
        os.environ['PM_SIGNATURE'] = 'sig'
        os.environ['PM_TIMESTAMP'] = '123'
        cfg = load_ws_auth_from_env()
        self.assertEqual(cfg['access_key'], 'kid')
        self.assertEqual(cfg['signature'], 'sig')
        self.assertEqual(cfg['timestamp'], '123')
