from __future__ import annotations

import time


def unix_timestamp_string() -> str:
    return str(int(time.time()))
