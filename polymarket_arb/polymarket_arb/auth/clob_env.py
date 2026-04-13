from __future__ import annotations

import os

from polymarket_arb.auth.clob_creds import ClobApiCreds


def load_clob_creds_from_env() -> ClobApiCreds | None:
    api_key = os.environ.get('CLOB_API_KEY')
    api_secret = os.environ.get('CLOB_SECRET')
    api_passphrase = os.environ.get('CLOB_PASS_PHRASE')
    if not api_key or not api_secret or not api_passphrase:
        return None
    return ClobApiCreds(api_key=api_key, api_secret=api_secret, api_passphrase=api_passphrase)
