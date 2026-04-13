import unittest

from polymarket_arb.monitoring.rules import classify_event


class MonitoringRulesTests(unittest.TestCase):
    def test_classifies_complete_ipo_cap_buckets(self) -> None:
        result = classify_event(
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
            }
        )
        self.assertTrue(result.passes_structure)
        self.assertEqual(result.structure_type, "complete_bucket")
        self.assertAlmostEqual(result.open_sum_ask, 0.999, places=3)

    def test_rejects_overlapping_above_thresholds(self) -> None:
        result = classify_event(
            {
                "slug": "consensys-ipo-closing-market-cap-above",
                "title": "Consensys IPO closing market cap above ___ ?",
                "showAllOutcomes": True,
                "markets": [
                    {"question": "Consensys IPO closing market cap above $1B?", "active": True, "closed": False, "bestAsk": 0.28},
                    {"question": "Consensys IPO closing market cap above $2B?", "active": True, "closed": False, "bestAsk": 0.27},
                    {"question": "Consensys IPO closing market cap above $3B?", "active": True, "closed": False, "bestAsk": 0.15},
                ],
            }
        )
        self.assertFalse(result.passes_structure)
        self.assertIn("overlap", result.reject_reasons)

    def test_rejects_overlapping_range_labels(self) -> None:
        result = classify_event(
            {
                "slug": "harvey-weinstein-prison-time",
                "title": "Harvey Weinstein prison time?",
                "showAllOutcomes": True,
                "markets": [
                    {"question": "Will Harvey Weinstein be sentenced to no prison time?", "active": True, "closed": False, "bestAsk": 0.263},
                    {"question": "Will Harvey Weinstein be sentenced to less than 5 years in prison?", "active": True, "closed": False, "bestAsk": 0.066},
                    {"question": "Will Harvey Weinstein be sentenced to between 5 and 10 years in prison?", "active": True, "closed": False, "bestAsk": 0.087},
                    {"question": "Will Harvey Weinstein be sentenced to between 10 and 20 years in prison?", "active": True, "closed": False, "bestAsk": 0.24},
                    {"question": "Will Harvey Weinstein be sentenced to between 20 and 30 years in prison?", "active": True, "closed": False, "bestAsk": 0.245},
                    {"question": "Will Harvey Weinstein be sentenced to more than 30 years in prison?", "active": True, "closed": False, "bestAsk": 0.148},
                ],
            }
        )
        self.assertFalse(result.passes_structure)
        self.assertIn("overlap", result.reject_reasons)

    def test_classifies_count_buckets_as_complete_but_not_positive_edge(self) -> None:
        result = classify_event(
            {
                "slug": "how-many-fed-rate-cuts-in-2026",
                "title": "How many Fed rate cuts in 2026?",
                "showAllOutcomes": True,
                "markets": [
                    {"question": "Will no Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.318},
                    {"question": "Will 1 Fed rate cut happen in 2026?", "active": True, "closed": False, "bestAsk": 0.27},
                    {"question": "Will 2 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.23},
                    {"question": "Will 3 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.12},
                    {"question": "Will 4 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.046},
                    {"question": "Will 5 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.017},
                    {"question": "Will 6 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.01},
                    {"question": "Will 7 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.006},
                    {"question": "Will 8 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.005},
                    {"question": "Will 9 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.004},
                    {"question": "Will 10 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.005},
                    {"question": "Will 11 Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.004},
                    {"question": "Will 12 or more Fed rate cuts happen in 2026?", "active": True, "closed": False, "bestAsk": 0.009},
                ],
            }
        )
        self.assertTrue(result.passes_structure)
        self.assertEqual(result.structure_type, "count_bucket")
        self.assertGreater(result.open_sum_ask, 1.0)

    def test_rejects_open_candidate_sets(self) -> None:
        result = classify_event(
            {
                "slug": "nobel-peace-prize-winner-2026-139",
                "title": "Nobel Peace Prize Winner 2026",
                "showAllOutcomes": True,
                "markets": [{"question": "Will Donald Trump win the Nobel Peace Prize in 2026?", "active": True, "closed": False, "bestAsk": 0.08}] * 20,
                "eventMetadata": {"market_count_total": 71},
            }
        )
        self.assertFalse(result.passes_structure)
        self.assertIn("open_set", result.reject_reasons)
