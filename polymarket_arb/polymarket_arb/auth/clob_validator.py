from __future__ import annotations

from polymarket_arb.auth.clob_creds import ClobApiCreds


def validate_clob_creds(creds: ClobApiCreds) -> list[str]:
    problems = []
    if not creds.api_key:
        problems.append('missing api_key')
    if not creds.api_secret:
        problems.append('missing api_secret')
    if not creds.api_passphrase:
        problems.append('missing api_passphrase')
    return problems
