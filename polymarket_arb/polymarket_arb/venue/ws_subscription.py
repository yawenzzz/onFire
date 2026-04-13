from __future__ import annotations


def build_market_subscription_message(asset_ids: list[str], operation: str | None = None, custom_feature_enabled: bool = True) -> dict:
    if not asset_ids:
        raise ValueError('asset_ids must not be empty')
    payload = {
        'assets_ids': asset_ids,
        'type': 'market',
        'custom_feature_enabled': custom_feature_enabled,
    }
    if operation is not None:
        payload['operation'] = operation
    return payload
