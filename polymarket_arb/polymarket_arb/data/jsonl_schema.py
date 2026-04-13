from __future__ import annotations

REQUIRED_CAPTURE_FIELDS = {'market_id', 'market_state', 'best_bid', 'best_ask'}


def validate_capture_record(record: dict) -> list[str]:
    missing = sorted(REQUIRED_CAPTURE_FIELDS - set(record.keys()))
    return [f'missing field: {name}' for name in missing]
