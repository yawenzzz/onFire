import unittest

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.shadow.certification import ShadowCertification
from polymarket_arb.shadow.metrics import ShadowMetrics


class ShadowCertificationTests(unittest.TestCase):
    def test_returns_blocked_when_surface_or_compliance_unresolved(self) -> None:
        verdict = ShadowCertification().evaluate(LaunchGate())
        self.assertEqual(verdict, "CERTIFICATION_BLOCKED")

    def test_returns_incomplete_when_metrics_not_good_enough(self) -> None:
        verdict = ShadowCertification().evaluate(
            LaunchGate(
                surface_resolved=True,
                surface_id="polymarket-us",
                jurisdiction_eligible=True,
                market_state_all_open=True,
                preview_success_rate=0.9,
                invalid_tick_or_price_reject_rate=0.0,
                api_429_count=0,
                ambiguous_rule_trade_count=0,
                collateral_return_dependency_for_safety=0,
                hedge_completion_rate_shadow=0.95,
                false_positive_rate=0.1,
                shadow_window_days=14,
            ),
            ShadowMetrics.from_counts(
                session_id="s1",
                surface_id="polymarket-us",
                preview_successes=9,
                preview_attempts=10,
                hedge_completions=9,
                hedge_attempts=10,
                false_positives=2,
                candidate_count=20,
                api_429_count=0,
                reconciliation_mismatch_count=0,
            ),
        )
        self.assertEqual(verdict, "CERTIFICATION_INCOMPLETE")

    def test_returns_live_capable_only_when_strict_shadow_gates_pass(self) -> None:
        verdict = ShadowCertification().evaluate(
            LaunchGate(
                surface_resolved=True,
                surface_id="polymarket-us",
                jurisdiction_eligible=True,
                market_state_all_open=True,
                preview_success_rate=1.0,
                invalid_tick_or_price_reject_rate=0.0,
                api_429_count=0,
                ambiguous_rule_trade_count=0,
                collateral_return_dependency_for_safety=0,
                hedge_completion_rate_shadow=0.995,
                false_positive_rate=0.02,
                shadow_window_days=14,
            ),
            ShadowMetrics.from_counts(
                session_id="s1",
                surface_id="polymarket-us",
                preview_successes=10,
                preview_attempts=10,
                hedge_completions=10,
                hedge_attempts=10,
                false_positives=1,
                candidate_count=100,
                api_429_count=0,
                reconciliation_mismatch_count=0,
            ),
        )
        self.assertEqual(verdict, "LIVE_CAPABLE_READY")

    def test_reconciliation_mismatch_blocks_live_capable(self) -> None:
        verdict = ShadowCertification().evaluate(
            LaunchGate(
                surface_resolved=True,
                surface_id="polymarket-us",
                jurisdiction_eligible=True,
                market_state_all_open=True,
                preview_success_rate=1.0,
                invalid_tick_or_price_reject_rate=0.0,
                api_429_count=0,
                ambiguous_rule_trade_count=0,
                collateral_return_dependency_for_safety=0,
                hedge_completion_rate_shadow=0.995,
                false_positive_rate=0.02,
                shadow_window_days=14,
            ),
            ShadowMetrics.from_counts(
                session_id="s1",
                surface_id="polymarket-us",
                preview_successes=10,
                preview_attempts=10,
                hedge_completions=10,
                hedge_attempts=10,
                false_positives=1,
                candidate_count=100,
                api_429_count=0,
                reconciliation_mismatch_count=1,
            ),
        )
        self.assertEqual(verdict, "CERTIFICATION_INCOMPLETE")
