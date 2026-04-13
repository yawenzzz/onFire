import unittest

from polymarket_arb.venue.backoff_policy import compute_backoff_seconds


class BackoffPolicyTests(unittest.TestCase):
    def test_backoff_increases_with_attempt(self) -> None:
        self.assertLess(compute_backoff_seconds(1), compute_backoff_seconds(2))
        self.assertLess(compute_backoff_seconds(2), compute_backoff_seconds(3))

    def test_backoff_is_capped(self) -> None:
        self.assertEqual(compute_backoff_seconds(100, base=1.0, cap=30.0), 30.0)
