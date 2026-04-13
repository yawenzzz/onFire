from __future__ import annotations

from dataclasses import dataclass


@dataclass
class HealthStatus:
    feed_fresh: bool
    archive_ok: bool
    parse_error_rate_ok: bool

    def healthy(self) -> bool:
        return self.feed_fresh and self.archive_ok and self.parse_error_rate_ok
