from __future__ import annotations


class IncidentWriter:
    def __init__(self) -> None:
        self.events: list[dict] = []

    def write(self, event) -> dict:
        payload = event.to_payload()
        self.events.append(payload)
        return payload
