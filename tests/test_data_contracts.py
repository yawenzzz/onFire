import unittest

from polymarket_arb.data.contracts import MarketSnapshot, PreviewPayload


class DataContractsTests(unittest.TestCase):
    def test_market_snapshot_has_tradeable_helper(self) -> None:
        snap = MarketSnapshot(market_id="m1", market_state="OPEN", best_bid=0.4, best_ask=0.5)
        self.assertTrue(snap.is_tradeable())

    def test_preview_payload_round_trips_to_dict(self) -> None:
        payload = PreviewPayload(price=0.5, quantity=1.0, side="BUY")
        self.assertEqual(payload.to_dict()["price"], 0.5)
