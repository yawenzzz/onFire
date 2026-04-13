import unittest

from polymarket_arb.venue.rtd_subscription import build_activity_trade_subscription, build_clob_user_subscription


class RTDSubscriptionTests(unittest.TestCase):
    def test_build_activity_trade_subscription(self) -> None:
        sub = build_activity_trade_subscription(event_slug='event-1')
        self.assertEqual(sub['topic'], 'activity')
        self.assertEqual(sub['type'], 'trades')
        self.assertIn('event_slug', sub['filters'])

    def test_build_clob_user_subscription(self) -> None:
        sub = build_clob_user_subscription('key', 'secret', 'pass')
        self.assertEqual(sub['topic'], 'clob_user')
        self.assertEqual(sub['clob_auth']['key'], 'key')
        self.assertEqual(sub['clob_auth']['secret'], 'secret')
        self.assertEqual(sub['clob_auth']['passphrase'], 'pass')
