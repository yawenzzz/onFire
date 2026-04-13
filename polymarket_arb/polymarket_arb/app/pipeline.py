from __future__ import annotations

from polymarket_arb.app.modes import choose_mode
from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.models.types import Leg
from polymarket_arb.rules.structure_parser import parse_structure
from polymarket_arb.shadow.certification import ShadowCertification
from polymarket_arb.shadow.certification_report import build_certification_report
from polymarket_arb.shadow.simulator import ShadowSimulator
from polymarket_arb.strategy.grouping import build_candidate_basket


def run_pipeline(
    session_id: str,
    surface_id: str,
    outcome_count: int,
    ordered_thresholds: bool,
    offset_relation: bool,
    legs: list[Leg],
    pi_min_stress_usd: float,
    hedge_completion_prob: float,
    capital_efficiency: float,
):
    structure = parse_structure(
        outcome_count=outcome_count,
        ordered_thresholds=ordered_thresholds,
        offset_relation=offset_relation,
    )
    if not structure.allowed:
        return ShadowSimulator().run(session_id=session_id, surface_id=surface_id, baskets=[])

    basket = build_candidate_basket(
        group_id="pipeline-g1",
        surface_id=surface_id,
        template_type=structure.template_type,
        legs=legs,
        pi_min_stress_usd=pi_min_stress_usd,
        hedge_completion_prob=hedge_completion_prob,
        capital_efficiency=capital_efficiency,
    )
    return ShadowSimulator().run(session_id=session_id, surface_id=surface_id, baskets=[basket])


def run_pipeline_report(
    session_id: str,
    surface_id: str,
    outcome_count: int,
    ordered_thresholds: bool,
    offset_relation: bool,
    legs: list[Leg],
    pi_min_stress_usd: float,
    hedge_completion_prob: float,
    capital_efficiency: float,
    surface_resolved: bool,
    jurisdiction_eligible: bool,
) -> dict:
    metrics = run_pipeline(
        session_id=session_id,
        surface_id=surface_id,
        outcome_count=outcome_count,
        ordered_thresholds=ordered_thresholds,
        offset_relation=offset_relation,
        legs=legs,
        pi_min_stress_usd=pi_min_stress_usd,
        hedge_completion_prob=hedge_completion_prob,
        capital_efficiency=capital_efficiency,
    )
    gate = LaunchGate(
        surface_resolved=surface_resolved,
        surface_id=surface_id,
        jurisdiction_eligible=jurisdiction_eligible,
        market_state_all_open=True,
        preview_success_rate=metrics.preview_success_rate,
        invalid_tick_or_price_reject_rate=0.0,
        api_429_count=metrics.api_429_count,
        ambiguous_rule_trade_count=0,
        collateral_return_dependency_for_safety=0,
        hedge_completion_rate_shadow=metrics.hedge_completion_rate_shadow,
        false_positive_rate=metrics.false_positive_rate,
        shadow_window_days=14,
    )
    verdict = ShadowCertification().evaluate(gate, metrics)
    return build_certification_report(verdict=verdict, metrics=metrics)


def choose_runtime_mode(surface_resolved: bool, certified: bool) -> str:
    return choose_mode(surface_resolved=surface_resolved, certified=certified)
