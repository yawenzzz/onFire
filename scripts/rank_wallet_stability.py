#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Tuple

EXCLUDED = {"wallet-filter-v1-summary.txt", "wallet-filter-v1-report.txt"}


@dataclass
class Candidate:
    wallet: str
    category: str
    source_report: str
    source_mtime: str
    fields: Dict[str, str] = field(default_factory=dict)
    stability_score: float = 0.0
    tier: str = "watch"
    rationale: str = ""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rank wallet-filter candidates by long-horizon stability using existing discovery reports.")
    parser.add_argument(
        "--discovery-dir",
        default="/Users/yawen.zheng/onFire/.omx/discovery",
        help="Directory containing wallet-filter-v1-*.txt reports",
    )
    parser.add_argument("--top", type=int, default=10, help="How many wallets to emit")
    parser.add_argument(
        "--out",
        default="/Users/yawen.zheng/onFire/.omx/discovery/wallet-stability-top10.txt",
        help="Report output path",
    )
    return parser.parse_args()


def to_float(value: str, default: float = 0.0) -> float:
    try:
        return float(value)
    except Exception:
        return default


def to_int(value: str, default: int = 0) -> int:
    try:
        return int(value)
    except Exception:
        return default


def parse_report(path: Path) -> List[Candidate]:
    source_mtime = dt.datetime.fromtimestamp(path.stat().st_mtime, tz=dt.timezone.utc).astimezone().isoformat()
    candidates: List[Candidate] = []
    current: Dict[str, str] | None = None
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith("== candidate ") and line.endswith(" =="):
            if current is not None:
                candidates.append(build_candidate(path, source_mtime, current))
            current = {}
            continue
        if current is None:
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        current[key] = value
    if current is not None:
        candidates.append(build_candidate(path, source_mtime, current))
    return candidates


def build_candidate(path: Path, source_mtime: str, fields: Dict[str, str]) -> Candidate:
    candidate = Candidate(
        wallet=fields.get("wallet", "unknown"),
        category=fields.get("category", "unknown"),
        source_report=path.name,
        source_mtime=source_mtime,
        fields=fields.copy(),
    )
    candidate.stability_score = score_candidate(candidate)
    candidate.tier = classify_tier(candidate)
    candidate.rationale = build_rationale(candidate)
    return candidate


def score_candidate(candidate: Candidate) -> float:
    f = candidate.fields
    score = 0.0
    review = f.get("review_status", "unknown")
    score += {"stable": 40.0, "downgrade": 15.0, "blacklist": -40.0}.get(review, 0.0)
    score += min(to_int(f.get("score_total", "0")), 100) * 0.4
    score -= to_int(f.get("maker_rebate_count", "0")) * 8.0
    score -= to_float(f.get("tail24", "0")) * 80.0
    score -= to_float(f.get("tail72", "0")) * 50.0
    score -= max(to_float(f.get("neg_risk_share", "0")) - 0.10, 0.0) * 120.0
    score -= max(0.70 - to_float(f.get("copyable_ratio", "0")), 0.0) * 60.0
    score -= max(to_float(f.get("flip60", "0")) - 0.10, 0.0) * 80.0
    score -= 8.0 if to_float(f.get("current_value_to_month_vol", "0")) <= 0.0 else 0.0
    median_hold = to_float(f.get("median_hold_hours", "0"))
    score += min(median_hold, 300.0) / 30.0
    uniq = to_int(f.get("unique_markets_90d", "0"))
    if uniq < 8:
        score -= 12.0
    elif uniq > 80:
        score -= 18.0
    elif uniq > 40:
        score -= 6.0
    score -= 10.0 if to_int(f.get("traded_markets", "0")) < 20 else 0.0
    score += max(0, 6 - to_int(f.get("week_rank", "999"))) * 1.5
    score += max(0, 6 - to_int(f.get("month_rank", "999"))) * 1.5
    return round(score, 2)


def classify_tier(candidate: Candidate) -> str:
    f = candidate.fields
    if (
        candidate.stability_score >= 70
        and f.get("review_status") == "stable"
        and to_int(f.get("maker_rebate_count", "0")) == 0
        and to_float(f.get("tail24", "0")) <= 0.10
        and to_float(f.get("tail72", "0")) <= 0.25
        and to_float(f.get("neg_risk_share", "0")) <= 0.10
        and to_float(f.get("copyable_ratio", "0")) >= 0.70
    ):
        return "core"
    if (
        candidate.stability_score >= 20
        and to_int(f.get("maker_rebate_count", "0")) == 0
        and to_float(f.get("neg_risk_share", "0")) <= 0.20
    ):
        return "watch"
    return "risky"


def build_rationale(candidate: Candidate) -> str:
    f = candidate.fields
    issues: List[str] = []
    if to_int(f.get("maker_rebate_count", "0")) > 0:
        issues.append("maker_like")
    if to_float(f.get("tail24", "0")) > 0.10 or to_float(f.get("tail72", "0")) > 0.25:
        issues.append("tail_heavy")
    if to_float(f.get("neg_risk_share", "0")) > 0.20:
        issues.append("high_neg_risk")
    if to_float(f.get("copyable_ratio", "0")) < 0.70:
        issues.append("low_copyable")
    uniq = to_int(f.get("unique_markets_90d", "0"))
    if uniq > 40:
        issues.append("too_broad")
    elif uniq < 8:
        issues.append("too_narrow")
    if to_int(f.get("traded_markets", "0")) < 20:
        issues.append("low_trade_count")
    if to_float(f.get("flip60", "0")) > 0.25:
        issues.append("high_flip")
    if not issues:
        return "cleanest current profile"
    return ",".join(issues)


def choose_best_by_wallet(candidates: List[Candidate]) -> List[Candidate]:
    best: Dict[str, Candidate] = {}
    for candidate in candidates:
        existing = best.get(candidate.wallet)
        if existing is None or candidate.stability_score > existing.stability_score:
            best[candidate.wallet] = candidate
    return sorted(
        best.values(),
        key=lambda c: (
            c.stability_score,
            {"core": 2, "watch": 1, "risky": 0}[c.tier],
            to_int(c.fields.get("score_total", "0")),
        ),
        reverse=True,
    )


def render_report(candidates: List[Candidate], top_n: int, discovery_dir: Path) -> str:
    now = dt.datetime.now().astimezone().isoformat()
    lines = [
        "wallet_stability_strategy=stability_rank_v1",
        f"generated_at={now}",
        f"discovery_dir={discovery_dir}",
        f"candidate_rows_total={len(candidates)}",
        f"unique_wallets_total={len({c.wallet for c in candidates})}",
        f"top_n={top_n}",
    ]
    for idx, candidate in enumerate(candidates[:top_n], start=1):
        f = candidate.fields
        lines.extend(
            [
                f"== wallet rank {idx} ==",
                f"wallet={candidate.wallet}",
                f"category={candidate.category}",
                f"tier={candidate.tier}",
                f"stability_score={candidate.stability_score:.2f}",
                f"source_report={candidate.source_report}",
                f"source_report_mtime={candidate.source_mtime}",
                f"review_status={f.get('review_status','unknown')}",
                f"review_reasons={f.get('review_reasons','none')}",
                f"week_rank={f.get('week_rank','na')}",
                f"month_rank={f.get('month_rank','na')}",
                f"score_total={f.get('score_total','na')}",
                f"month_pnl={f.get('month_pnl','na')}",
                f"maker_rebate_count={f.get('maker_rebate_count','na')}",
                f"flip60={f.get('flip60','na')}",
                f"median_hold_hours={f.get('median_hold_hours','na')}",
                f"p75_hold_hours={f.get('p75_hold_hours','na')}",
                f"tail24={f.get('tail24','na')}",
                f"tail72={f.get('tail72','na')}",
                f"neg_risk_share={f.get('neg_risk_share','na')}",
                f"copyable_ratio={f.get('copyable_ratio','na')}",
                f"unique_markets_90d={f.get('unique_markets_90d','na')}",
                f"traded_markets={f.get('traded_markets','na')}",
                f"current_value_to_month_vol={f.get('current_value_to_month_vol','na')}",
                f"rejection_reasons={f.get('rejection_reasons','none')}",
                f"rationale={candidate.rationale}",
            ]
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    discovery_dir = Path(args.discovery_dir)
    reports = sorted(
        path for path in discovery_dir.glob("wallet-filter-v1-*.txt") if path.name not in EXCLUDED
    )
    if not reports:
        raise SystemExit(f"no wallet-filter reports found under {discovery_dir}")

    all_candidates: List[Candidate] = []
    for report in reports:
        all_candidates.extend(parse_report(report))
    ranked = choose_best_by_wallet(all_candidates)
    report_body = render_report(ranked, args.top, discovery_dir)
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(report_body)
    print(f"wallet_stability_report_path={out_path}")
    print(f"wallet_stability_top_wallet={ranked[0].wallet if ranked else 'none'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
