import unittest

from polymarket_arb.monitoring.models import EventScanResult
from polymarket_arb.monitoring.order_recommendation import recommend_order_draft


class OrderRecommendationTests(unittest.TestCase):
    def test_recommends_bundle_from_top_candidate(self) -> None:
        snapshot = {
            "candidates": [
                {
                    "title": "Edge But Weak Net",
                    "template_type": "market_cap_buckets",
                    "category": "Politics",
                    "gross_edge": 0.04,
                    "adjusted_edge": 0.01,
                    "recommended_orders": [
                        {"token_id": "t1", "side": "BUY", "price": 0.48, "size": 5.0, "order_min_size": 5.0},
                        {"token_id": "t2", "side": "BUY", "price": 0.48, "size": 5.0, "order_min_size": 5.0},
                    ],
                },
                {
                    "title": "Best Event",
                    "template_type": "market_cap_buckets",
                    "category": "Politics",
                    "gross_edge": 0.08,
                    "adjusted_edge": 0.05,
                    "recommended_orders": [
                        {"token_id": "t3", "side": "BUY", "price": 0.45, "size": 5.0, "order_min_size": 5.0},
                        {"token_id": "t4", "side": "BUY", "price": 0.47, "size": 5.0, "order_min_size": 5.0},
                    ],
                }
            ]
        }

        draft = recommend_order_draft(snapshot)

        self.assertIsNotNone(draft)
        assert draft is not None
        self.assertEqual(draft["title"], "Best Event")
        self.assertEqual(len(draft["legs"]), 2)
        self.assertEqual(draft["bundle_size"], 5.0)
        self.assertAlmostEqual(draft["total_price"], 0.92)
        self.assertGreater(draft["fee_cost"], 0)
        self.assertAlmostEqual(draft["net_edge"], 0.029697, places=5)

    def test_returns_none_without_candidates(self) -> None:
        self.assertIsNone(recommend_order_draft({"candidates": []}))

    def test_returns_none_when_net_edge_not_positive(self) -> None:
        snapshot = {
            "candidates": [
                {
                    "title": "No Edge",
                    "template_type": "market_cap_buckets",
                    "category": "Politics",
                    "gross_edge": 0.02,
                    "recommended_orders": [
                        {"token_id": "t1", "side": "BUY", "price": 0.49, "size": 1.0, "order_min_size": 1.0},
                        {"token_id": "t2", "side": "BUY", "price": 0.49, "size": 1.0, "order_min_size": 1.0},
                    ],
                }
            ]
        }
        self.assertIsNone(recommend_order_draft(snapshot))

    def test_supports_object_candidates(self) -> None:
        candidate = EventScanResult(
            slug="best-event",
            title="Best Event",
            template_type="market_cap_buckets",
            category="Politics",
            is_complete=True,
            open_market_count=2,
            total_market_count=2,
            sum_ask=0.5,
            gross_edge=0.08,
            cost_adjusted_edge=0.05,
            rejection_reason=None,
            questions=[],
            recommended_orders=[
                {"token_id": "t3", "side": "BUY", "price": 0.45, "size": 5.0, "order_min_size": 5.0},
                {"token_id": "t4", "side": "BUY", "price": 0.47, "size": 5.0, "order_min_size": 5.0},
            ],
        )

        draft = recommend_order_draft({"candidates": [candidate]})

        self.assertIsNotNone(draft)
        assert draft is not None
        self.assertEqual(draft["title"], "Best Event")
