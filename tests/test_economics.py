import unittest
from decimal import Decimal

from polymarket_arb.monitoring.economics import (
    buy_cost_for_net_shares,
    fee_adjusted_bundle_cost,
    fee_rate_for_candidate,
)


class EconomicsTests(unittest.TestCase):
    def test_fee_rate_for_candidate_uses_category(self) -> None:
        self.assertEqual(fee_rate_for_candidate({"category": "Politics"}), Decimal("0.04"))
        self.assertEqual(fee_rate_for_candidate({"category": "Geopolitics"}), Decimal("0"))

    def test_buy_cost_for_net_shares_is_higher_than_raw_cost_when_fee_positive(self) -> None:
        raw = Decimal("1") * Decimal("0.45")
        adjusted = buy_cost_for_net_shares(Decimal("1"), Decimal("0.45"), Decimal("0.04"))
        self.assertGreater(adjusted, raw)

    def test_fee_adjusted_bundle_cost_aggregates_leg_costs(self) -> None:
        cost = fee_adjusted_bundle_cost(Decimal("1"), [Decimal("0.45"), Decimal("0.47")], Decimal("0.04"))
        self.assertGreater(cost, Decimal("0.92"))
