import unittest

from polymarket_arb.monitoring.rule_engine import evaluate_complete_bucket_event


def _event(title: str, questions: list[str]) -> dict:
    return {
        "title": title,
        "markets": [
            {
                "question": question,
                "active": True,
                "closed": False,
                "bestAsk": 0.1,
                "bestBid": 0.09,
            }
            for question in questions
        ],
    }


class MonitorRuleEngineTests(unittest.TestCase):
    def test_accepts_ipo_market_cap_complete_bucket(self) -> None:
        result = evaluate_complete_bucket_event(
            _event(
                "Discord IPO Closing Market Cap",
                [
                    "Will Discord's market cap be less than $15B at market close on IPO day?",
                    "Will Discord's market cap be between $15B and $20B at market close on IPO day?",
                    "Will Discord's market cap be between $20B and $25B at market close on IPO day?",
                    "Will Discord's market cap be between $25B and $30B at market close on IPO day?",
                    "Will Discord's market cap be $30B or greater at market close on IPO day?",
                    "Will Discord not IPO by June 30, 2026?",
                ],
            )
        )
        self.assertTrue(result.is_candidate)
        self.assertEqual(result.template_type, "ipo_market_cap_complete_bucket")

    def test_rejects_overlapping_thresholds(self) -> None:
        result = evaluate_complete_bucket_event(
            _event(
                "Consensys IPO closing market cap above ___ ?",
                [
                    "Consensys IPO closing market cap above $1B?",
                    "Consensys IPO closing market cap above $2B?",
                    "Consensys IPO closing market cap above $3B?",
                ],
            )
        )
        self.assertFalse(result.is_candidate)
        self.assertEqual(result.rejection_reason, "overlapping_thresholds")

    def test_rejects_overlapping_ranges(self) -> None:
        result = evaluate_complete_bucket_event(
            _event(
                "Harvey Weinstein prison time?",
                [
                    "Will Harvey Weinstein be sentenced to no prison time?",
                    "Will Harvey Weinstein be sentenced to less than 5 years in prison?",
                    "Will Harvey Weinstein be sentenced to between 5 and 10 years in prison?",
                    "Will Harvey Weinstein be sentenced to between 10 and 20 years in prison?",
                    "Will Harvey Weinstein be sentenced to between 20 and 30 years in prison?",
                    "Will Harvey Weinstein be sentenced to more than 30 years in prison?",
                ],
            )
        )
        self.assertFalse(result.is_candidate)
        self.assertEqual(result.rejection_reason, "overlapping_ranges")

    def test_accepts_count_buckets_with_open_tail(self) -> None:
        result = evaluate_complete_bucket_event(
            _event(
                "How many Fed rate cuts in 2026?",
                [
                    "Will no Fed rate cuts happen in 2026?",
                    "Will 1 Fed rate cut happen in 2026?",
                    "Will 2 Fed rate cuts happen in 2026?",
                    "Will 3 Fed rate cuts happen in 2026?",
                    "Will 4 Fed rate cuts happen in 2026?",
                    "Will 5 Fed rate cuts happen in 2026?",
                    "Will 6 Fed rate cuts happen in 2026?",
                    "Will 7 Fed rate cuts happen in 2026?",
                    "Will 8 Fed rate cuts happen in 2026?",
                    "Will 9 Fed rate cuts happen in 2026?",
                    "Will 10 Fed rate cuts happen in 2026?",
                    "Will 11 Fed rate cuts happen in 2026?",
                    "Will 12 or more Fed rate cuts happen in 2026?",
                ],
            )
        )
        self.assertTrue(result.is_candidate)
        self.assertEqual(result.template_type, "count_complete_bucket")

    def test_rejects_open_candidate_set(self) -> None:
        result = evaluate_complete_bucket_event(
            _event(
                "Nobel Peace Prize Winner 2026",
                [
                    "Will Donald Trump win the Nobel Peace Prize in 2026?",
                    "Will Greta Thunberg win the Nobel Peace Prize in 2026?",
                    "Will António Guterres win the Nobel Peace Prize in 2026?",
                ],
            )
        )
        self.assertFalse(result.is_candidate)
        self.assertEqual(result.rejection_reason, "open_candidate_set")
