import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.source_loader import load_market_snapshots


class SourceLoaderTests(unittest.TestCase):
    def test_loads_market_snapshots_from_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'markets.json'
            path.write_text(json.dumps([
                {"market_id": "m1", "market_state": "OPEN", "best_bid": 0.4, "best_ask": 0.5}
            ]))
            snaps = load_market_snapshots(path)
            self.assertEqual(len(snaps), 1)
            self.assertEqual(snaps[0].market_id, 'm1')
