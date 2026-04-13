from __future__ import annotations

from dataclasses import dataclass


@dataclass
class ClobApiCreds:
    api_key: str | None = None
    api_secret: str | None = None
    api_passphrase: str | None = None

    def is_complete(self) -> bool:
        return all([self.api_key, self.api_secret, self.api_passphrase])
