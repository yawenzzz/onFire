import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.jsonl_market_loader import load_market_snapshots_jsonl


class JsonlMarketLoaderTests(unittest.TestCase):
    def test_loads_multiple_snapshots_from_jsonl(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'capture.jsonl'
            path.write_text(
                '{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}\n'
                '{"market_id":"m2","market_state":"OPEN","best_bid":0.3,"best_ask":0.6}\n'
            )
            snaps = load_market_snapshots_jsonl(path)
            self.assertEqual(len(snaps), 2)
            self.assertEqual(snaps[1].market_id, 'm2')
