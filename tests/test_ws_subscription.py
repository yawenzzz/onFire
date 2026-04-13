import unittest

from polymarket_arb.venue.ws_subscription import build_market_subscription_message


class WSSubscriptionTests(unittest.TestCase):
    def test_builds_subscription_message_for_asset_ids(self) -> None:
        msg = build_market_subscription_message(['m1', 'm2'])
        self.assertEqual(msg['type'], 'market')
        self.assertEqual(msg['assets_ids'], ['m1', 'm2'])
        self.assertTrue(msg['custom_feature_enabled'])

    def test_rejects_empty_asset_id_list(self) -> None:
        with self.assertRaises(ValueError):
            build_market_subscription_message([])
