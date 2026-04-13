import unittest

from polymarket_arb.monitoring.rules import classify_complete_bucket_event


def _market(question: str, ask: float, active: bool = True, closed: bool = False) -> dict:
    return {
        "question": question,
        "bestAsk": ask,
        "active": active,
        "closed": closed,
    }


class MonitorRulesTests(unittest.TestCase):
    def test_accepts_complete_market_cap_buckets(self) -> None:
        event = {
            "slug": "discord-ipo-closing-market-cap",
            "title": "Discord IPO Closing Market Cap",
            "showAllOutcomes": True,
            "markets": [
                _market("Will Discord's market cap be less than $15B at market close on IPO day?", 0.12),
                _market("Will Discord's market cap be between $15B and $20B at market close on IPO day?", 0.02),
                _market("Will Discord's market cap be between $20B and $25B at market close on IPO day?", 0.01),
                _market("Will Discord's market cap be between $25B and $30B at market close on IPO day?", 0.01),
                _market("Will Discord's market cap be $30B or greater at market close on IPO day?", 0.03),
                _market("Will Discord not IPO by June 30, 2026?", 0.80),
            ],
        }

        result = classify_complete_bucket_event(event, min_edge=0.03)

        self.assertTrue(result.is_complete)
        self.assertEqual(result.template_type, "market_cap_buckets")
        self.assertAlmostEqual(result.sum_ask, 0.99)
        self.assertAlmostEqual(result.gross_edge, 0.01)
        self.assertEqual(result.rejection_reason, "edge_below_threshold")

    def test_rejects_overlapping_threshold_buckets(self) -> None:
        event = {
            "slug": "consensys-ipo-closing-market-cap-above",
            "title": "Consensys IPO closing market cap above ___ ?",
            "showAllOutcomes": True,
            "markets": [
                _market("Consensys IPO closing market cap above $1B?", 0.28),
                _market("Consensys IPO closing market cap above $2B?", 0.27),
                _market("Consensys IPO closing market cap above $3B?", 0.15),
            ],
        }

        result = classify_complete_bucket_event(event, min_edge=0.03)

        self.assertFalse(result.is_complete)
        self.assertEqual(result.rejection_reason, "overlapping_thresholds")

    def test_rejects_open_candidate_set_even_when_sum_ask_is_low(self) -> None:
        event = {
            "slug": "nobel-peace-prize-winner-2026",
            "title": "Nobel Peace Prize Winner 2026",
            "showAllOutcomes": True,
            "markets": [
                _market("Will Donald Trump win the Nobel Peace Prize in 2026?", 0.08),
                _market("Will Yulia Navalnaya win the Nobel Peace Prize in 2026?", 0.11),
                _market("Will Greta Thunberg win the Nobel Peace Prize in 2026?", 0.03),
            ] + [
                _market(f"Closed candidate {i}", 0.01, active=True, closed=True)
                for i in range(68)
            ],
        }

        result = classify_complete_bucket_event(event, min_edge=0.03)

        self.assertFalse(result.is_complete)
        self.assertEqual(result.rejection_reason, "open_candidate_set")
