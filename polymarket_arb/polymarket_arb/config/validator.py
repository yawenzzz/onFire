from __future__ import annotations

from polymarket_arb.config.schemas import LaunchGate


def validate_launch_gate(gate: LaunchGate) -> list[str]:
    problems: list[str] = []
    if not gate.surface_resolved and gate.surface_id != "unresolved":
        problems.append("surface_resolved is false but surface_id is set")
    if gate.surface_resolved and gate.surface_id == "unresolved":
        problems.append("surface_resolved is true but surface_id is unresolved")
    return problems
