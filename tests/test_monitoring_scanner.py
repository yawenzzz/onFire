import unittest

from polymarket_arb.monitoring.scanner import GammaScanner


class StubGammaClient:
    def __init__(self, events):
        self.events = events

    def fetch_events(self, limit: int = 50, closed: bool = False):
        return self.events[:limit]


class MonitoringScannerTests(unittest.TestCase):
    def test_scanner_returns_candidates_and_rejections(self) -> None:
        scanner = GammaScanner(
            client=StubGammaClient(
                [
                    {
                        "slug": "fannie-mae-ipo-closing-market-cap",
                        "title": "Fannie Mae IPO Closing Market Cap",
                        "showAllOutcomes": True,
                        "markets": [
                            {"question": "Will Fannie Mae’s market cap be less than $200B at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.003},
                            {"question": "Will Fannie Mae’s market cap be between $200B and $250B at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.003},
                            {"question": "Will Fannie Mae’s market cap be between $250B and $300B at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.003},
                            {"question": "Will Fannie Mae’s market cap be between $300B and $350B at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.004},
                            {"question": "Will Fannie Mae’s market cap be between $350B and $400B at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.005},
                            {"question": "Will Fannie Mae’s market cap be $400B or greater at market close on IPO day?", "active": True, "closed": False, "bestAsk": 0.003},
                            {"question": "Will Fannie Mae not IPO by June 30, 2026?", "active": True, "closed": False, "bestAsk": 0.978},
                        ],
                    },
                    {
                        "slug": "consensys-ipo-closing-market-cap-above",
                        "title": "Consensys IPO closing market cap above ___ ?",
                        "showAllOutcomes": True,
                        "markets": [
                            {"question": "Consensys IPO closing market cap above $1B?", "active": True, "closed": False, "bestAsk": 0.28},
                            {"question": "Consensys IPO closing market cap above $2B?", "active": True, "closed": False, "bestAsk": 0.27},
                            {"question": "Consensys IPO closing market cap above $3B?", "active": True, "closed": False, "bestAsk": 0.15},
                        ],
                    },
                ]
            ),
            min_edge_threshold=0.0,
        )
        snapshot = scanner.scan(limit=10)
        self.assertEqual(snapshot.event_count, 2)
        self.assertEqual(len(snapshot.candidates), 1)
        self.assertEqual(snapshot.candidates[0].slug, "fannie-mae-ipo-closing-market-cap")
        self.assertEqual(len(snapshot.rejections), 1)
        self.assertIn("overlap", snapshot.rejections[0].reject_reasons)
