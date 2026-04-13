from __future__ import annotations


def choose_mode(surface_resolved: bool, certified: bool) -> str:
    if not surface_resolved:
        return "research"
    if not certified:
        return "shadow"
    return "live_capable"
