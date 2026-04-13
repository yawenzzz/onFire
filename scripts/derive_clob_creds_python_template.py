"""Template: derive or create Polymarket CLOB API credentials locally.

Requires local installation of py-clob-client and your own private key/funder info.
Never commit or share real secrets.
"""

import json
import os
import sys

try:
    from py_clob_client.client import ClobClient
except Exception as exc:  # pragma: no cover - local dependency opt-in
    print(f"py-clob-client import failed: {exc}", file=sys.stderr)
    sys.exit(1)


def main() -> int:
    host = os.environ.get("CLOB_HOST", "https://clob.polymarket.com")
    chain_id = int(os.environ.get("CHAIN_ID", "137"))
    private_key = os.environ.get("PRIVATE_KEY")
    if not private_key:
        print("Missing PRIVATE_KEY in env", file=sys.stderr)
        return 2

    signature_type = int(os.environ.get("SIGNATURE_TYPE", "0"))
    funder = os.environ.get("FUNDER")

    client = ClobClient(
        host,
        key=private_key,
        chain_id=chain_id,
        signature_type=signature_type,
        funder=funder,
    )
    creds = client.create_or_derive_api_creds()
    print(json.dumps({
        "CLOB_API_KEY": creds.api_key,
        "CLOB_SECRET": creds.api_secret,
        "CLOB_PASS_PHRASE": creds.api_passphrase,
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
