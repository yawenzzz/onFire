from __future__ import annotations


def build_activity_trade_subscription(event_slug: str) -> dict:
    return {
        'topic': 'activity',
        'type': 'trades',
        'filters': {'event_slug': event_slug},
    }


def build_clob_user_subscription(key: str, secret: str, passphrase: str) -> dict:
    return {
        'topic': 'clob_user',
        'type': '*',
        'clob_auth': {
            'key': key,
            'secret': secret,
            'passphrase': passphrase,
        },
    }
