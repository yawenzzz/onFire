import unittest

from polymarket_arb.auth.clob_creds import ClobApiCreds
from polymarket_arb.auth.clob_validator import validate_clob_creds


class ClobValidatorTests(unittest.TestCase):
    def test_reports_missing_fields(self) -> None:
        problems = validate_clob_creds(ClobApiCreds(api_key='k'))
        self.assertIn('missing api_secret', problems)
        self.assertIn('missing api_passphrase', problems)

    def test_complete_creds_have_no_problems(self) -> None:
        problems = validate_clob_creds(ClobApiCreds(api_key='k', api_secret='s', api_passphrase='p'))
        self.assertEqual(problems, [])
