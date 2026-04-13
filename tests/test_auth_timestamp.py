import unittest

from polymarket_arb.auth.timestamp import unix_timestamp_string


class AuthTimestampTests(unittest.TestCase):
    def test_returns_digit_string(self) -> None:
        ts = unix_timestamp_string()
        self.assertTrue(ts.isdigit())
