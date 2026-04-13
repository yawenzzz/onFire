from __future__ import annotations

from dataclasses import dataclass, field

from polymarket_arb.models.types import CandidateBasket
from polymarket_arb.rules.drift_guard import DriftDecision


@dataclass
class ScoreOutput:
    group_id: str
    hard_gate_passed: bool
    score_raw: float
    rank_score: float
    rejected: bool
    reject_reasons: list[str] = field(default_factory=list)


class RobustScorer:
    def score(
        self,
        basket: CandidateBasket,
        drift: DriftDecision | None = None,
    ) -> ScoreOutput:
        reject_reasons: list[str] = []
        if not basket.is_structurally_safe():
            reject_reasons.append("structural gate failed")
        if basket.pi_min_stress_usd <= 0:
            reject_reasons.append("non-positive stressed payoff")
        if not basket.zero_rebate_positive:
            reject_reasons.append("reward-dependent profitability")
        if basket.hedge_completion_prob < 0.99:
            reject_reasons.append("hedge completion below threshold")
        if drift is not None and not drift.allowed:
            reject_reasons.extend(drift.reasons)

        if reject_reasons:
            return ScoreOutput(
                group_id=basket.group_id,
                hard_gate_passed=False,
                score_raw=float("-inf"),
                rank_score=float("-inf"),
                rejected=True,
                reject_reasons=reject_reasons,
            )

        score_raw = (
            basket.pi_min_stress_usd
            * basket.hedge_completion_prob
            * basket.capital_efficiency
        )
        rank_score = score_raw - basket.ambiguity_penalty - basket.ops_penalty
        return ScoreOutput(
            group_id=basket.group_id,
            hard_gate_passed=True,
            score_raw=score_raw,
            rank_score=rank_score,
            rejected=False,
        )
