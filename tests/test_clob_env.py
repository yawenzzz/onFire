import os
import unittest

from polymarket_arb.auth.clob_env import load_clob_creds_from_env


class ClobEnvTests(unittest.TestCase):
    def test_returns_none_when_missing(self) -> None:
        for key in ['CLOB_API_KEY', 'CLOB_SECRET', 'CLOB_PASS_PHRASE']:
            os.environ.pop(key, None)
        self.assertIsNone(load_clob_creds_from_env())

    def test_loads_credential_triplet(self) -> None:
        os.environ['CLOB_API_KEY'] = 'k'
        os.environ['CLOB_SECRET'] = 's'
        os.environ['CLOB_PASS_PHRASE'] = 'p'
        creds = load_clob_creds_from_env()
        self.assertEqual(creds.api_key, 'k')
        self.assertEqual(creds.api_secret, 's')
        self.assertEqual(creds.api_passphrase, 'p')
