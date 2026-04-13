import unittest

from polymarket_arb.venue.preview_client import PreviewClient, PreviewResult


class PreviewClientTests(unittest.TestCase):
    def test_fail_closed_on_invalid_price_bounds(self) -> None:
        client = PreviewClient()
        result = client.preview(price=1.2, tick_valid=True, market_open=True)
        self.assertIsInstance(result, PreviewResult)
        self.assertFalse(result.ok)
        self.assertEqual(result.reason, "price out of bounds")

    def test_fail_closed_on_market_not_open(self) -> None:
        client = PreviewClient()
        result = client.preview(price=0.5, tick_valid=True, market_open=False)
        self.assertFalse(result.ok)
        self.assertEqual(result.reason, "market not open")

    def test_accepts_valid_preview_input(self) -> None:
        client = PreviewClient()
        result = client.preview(price=0.5, tick_valid=True, market_open=True)
        self.assertTrue(result.ok)
        self.assertIsNone(result.reason)
