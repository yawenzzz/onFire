from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256


@dataclass(frozen=True)
class RuleSnapshot:
    text_hash: str

    @classmethod
    def from_text(cls, text: str) -> "RuleSnapshot":
        return cls(text_hash=sha256(text.encode("utf-8")).hexdigest())

    def matches_text(self, text: str) -> bool:
        return self.text_hash == sha256(text.encode("utf-8")).hexdigest()
