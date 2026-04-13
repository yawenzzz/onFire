import unittest

from polymarket_arb.auth.clob_creds import ClobApiCreds


class ClobCredsTests(unittest.TestCase):
    def test_complete_only_when_all_fields_present(self) -> None:
        self.assertFalse(ClobApiCreds(api_key='k').is_complete())
        self.assertTrue(ClobApiCreds(api_key='k', api_secret='s', api_passphrase='p').is_complete())
