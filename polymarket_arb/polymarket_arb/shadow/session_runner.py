from __future__ import annotations

from polymarket_arb.app.cli import build_cli_summary
from polymarket_arb.app.pipeline import run_pipeline_report
from polymarket_arb.models.types import Leg


def run_shadow_session(
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
):
    report = run_pipeline_report(
        session_id=session_id,
        surface_id=surface_id,
        outcome_count=outcome_count,
        ordered_thresholds=ordered_thresholds,
        offset_relation=offset_relation,
        legs=legs,
        pi_min_stress_usd=pi_min_stress_usd,
        hedge_completion_prob=hedge_completion_prob,
        capital_efficiency=capital_efficiency,
        surface_resolved=surface_resolved,
        jurisdiction_eligible=jurisdiction_eligible,
    )
    summary = build_cli_summary(mode="shadow", verdict=report["verdict"])
    return report, summary
