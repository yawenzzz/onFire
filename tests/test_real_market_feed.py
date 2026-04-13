import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.source_loader import load_market_snapshots
from polymarket_arb.venue.real_market_feed import LocalMarketFeed


class RealMarketFeedTests(unittest.TestCase):
    def test_reads_snapshots_via_loader(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'markets.json'
            path.write_text('[{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}]')
            feed = LocalMarketFeed(path, loader=load_market_snapshots)
            snaps = feed.read()
            self.assertEqual(snaps[0].market_id, 'm1')
