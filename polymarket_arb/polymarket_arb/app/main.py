from __future__ import annotations

from polymarket_arb.app.bootstrap import startup_posture
from polymarket_arb.config.schemas import LaunchGate
from polymarket_arb.venue.surface_gate import SurfaceGate


def run_app(surface_gate: SurfaceGate, launch_gate: LaunchGate) -> str:
    return startup_posture(surface_gate, launch_gate)
