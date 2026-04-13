from __future__ import annotations

import argparse
from pathlib import Path

from polymarket_arb.data.input_schema import load_shadow_input
from polymarket_arb.models.types import Leg, MarketState
from polymarket_arb.ops.file_reporter import write_json_report
from polymarket_arb.shadow.dashboard_schema import build_dashboard_payload
from polymarket_arb.shadow.report_archive import archive_bundle
from polymarket_arb.shadow.session_runner import run_shadow_session


def _legs_from_raw(raw_legs: list[dict]) -> list[Leg]:
    return [
        Leg(
            item['market_id'],
            item['side'],
            item['price'],
            MarketState(item['market_state']),
            item['tick_valid'],
            item['visible_depth_qty'],
            item['preview_ok'],
            item['clarification_hash'],
        )
        for item in raw_legs
    ]


def run_shadow_entrypoint(
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
) -> str:
    _, summary = run_shadow_session(
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
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--session-id')
    parser.add_argument('--surface-id')
    parser.add_argument('--outcome-count', type=int)
    parser.add_argument('--ordered-thresholds', action='store_true')
    parser.add_argument('--offset-relation', action='store_true')
    parser.add_argument('--surface-resolved', action='store_true')
    parser.add_argument('--jurisdiction-eligible', action='store_true')
    parser.add_argument('--input-file')
    parser.add_argument('--archive-root')
    parser.add_argument('--output', required=True)
    args = parser.parse_args(argv)

    if args.input_file:
        data = load_shadow_input(Path(args.input_file))
        session_id = data['session_id']
        surface_id = data['surface_id']
        outcome_count = data['outcome_count']
        ordered_thresholds = data['ordered_thresholds']
        offset_relation = data['offset_relation']
        legs = _legs_from_raw(data['legs'])
        pi_min_stress_usd = data['pi_min_stress_usd']
        hedge_completion_prob = data['hedge_completion_prob']
        capital_efficiency = data['capital_efficiency']
        surface_resolved = data['surface_resolved']
        jurisdiction_eligible = data['jurisdiction_eligible']
    else:
        if args.session_id is None or args.surface_id is None or args.outcome_count is None:
            parser.error('the following arguments are required: --session-id, --surface-id, --outcome-count')
        session_id = args.session_id
        surface_id = args.surface_id
        outcome_count = args.outcome_count
        ordered_thresholds = args.ordered_thresholds
        offset_relation = args.offset_relation
        surface_resolved = args.surface_resolved
        jurisdiction_eligible = args.jurisdiction_eligible
        legs = [Leg('m1', 'BUY', 0.4, MarketState.OPEN, True, 10, True, 'a')]
        pi_min_stress_usd = 1.0
        hedge_completion_prob = 0.99
        capital_efficiency = 0.5

    report, summary = run_shadow_session(
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
    write_json_report(args.output, report)
    if args.archive_root:
        archive_bundle(args.archive_root, session_id, report, summary, build_dashboard_payload(report_to_metrics(report)))
    return 0


def report_to_metrics(report: dict):
    from polymarket_arb.shadow.metrics import ShadowMetrics
    return ShadowMetrics(
        session_id=report['session_id'],
        surface_id=report['surface_id'],
        preview_success_rate=report['preview_success_rate'],
        hedge_completion_rate_shadow=report['hedge_completion_rate_shadow'],
        false_positive_rate=report['false_positive_rate'],
        api_429_count=report['api_429_count'],
        reconciliation_mismatch_count=report['reconciliation_mismatch_count'],
    )


if __name__ == '__main__':
    raise SystemExit(main())
