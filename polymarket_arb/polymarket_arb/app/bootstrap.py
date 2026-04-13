from __future__ import annotations

from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.venue.surface_gate import SurfaceGate


def startup_posture(surface_gate: SurfaceGate, launch_gate: LaunchGate) -> str:
    if surface_gate.fail_reason() is not None:
        return "NO_TRADE"
    return launch_gate.posture()
