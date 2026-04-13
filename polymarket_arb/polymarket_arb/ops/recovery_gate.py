from __future__ import annotations


class RecoveryGate:
    def allow_restart(self, has_open_critical_incident: bool, reconciliation_clean: bool) -> bool:
        return not has_open_critical_incident and reconciliation_clean
