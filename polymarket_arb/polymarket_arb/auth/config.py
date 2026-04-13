from __future__ import annotations

from dataclasses import dataclass


@dataclass
class WsAuthConfig:
    access_key: str | None = None
    signature: str | None = None
    timestamp: str | None = None

    def is_complete(self) -> bool:
        return all([self.access_key, self.signature, self.timestamp])
