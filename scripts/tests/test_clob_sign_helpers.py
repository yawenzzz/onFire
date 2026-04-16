import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = ROOT / "scripts"


ORDER_PAYLOAD = {
    "maker": "0xfunder-address",
    "signer": "0xpoly-address",
    "signatureType": 2,
    "taker": "0x0000000000000000000000000000000000000000",
    "tokenId": "12345",
    "makerAmount": "1000000",
    "takerAmount": "2000000",
    "side": "BUY",
    "expiration": "1735689600",
    "nonce": "7",
    "feeRateBps": "30",
}

L2_PAYLOAD = {
    "address": "0xpoly-address",
    "method": "POST",
    "requestPath": "/orders",
    "body": '{"owner":"owner-uuid"}',
}

BASE_ENV = {
    "POLY_ADDRESS": "0xpoly-address",
    "CLOB_API_KEY": "api-key",
    "CLOB_SECRET": "api-secret",
    "CLOB_PASS_PHRASE": "passphrase",
    "PRIVATE_KEY": "private-key",
    "SIGNATURE_TYPE": "2",
    "FUNDER_ADDRESS": "0xfunder-address",
    "CHAIN_ID": "137",
}


def run_script(script_name: str, payload: dict, env: dict) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPTS_DIR / script_name), "--json"],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        cwd=ROOT,
        env={**os.environ, **env},
    )


def read_log(log_path: Path) -> list[dict]:
    if not log_path.exists():
        return []
    return [json.loads(line) for line in log_path.read_text().splitlines() if line.strip()]


class RepoLocalHelperScriptsTest(unittest.TestCase):
    maxDiff = None

    def write_stub_sdk(self, stub_root: Path) -> None:
        files = {
            "stub_log.py": '''
import json
import os
from pathlib import Path


def append(event):
    path = os.environ.get("STUB_LOG_PATH")
    if not path:
        return
    with Path(path).open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(event, sort_keys=True) + "\\n")
''',
            "py_clob_client/__init__.py": "",
            "py_clob_client/config.py": '''
from types import SimpleNamespace
from stub_log import append


def get_contract_config(chain_id, neg_risk=False):
    append({"event": "get_contract_config", "chain_id": chain_id, "neg_risk": neg_risk})
    return SimpleNamespace(exchange="0xexchange")
''',
            "py_clob_client/signer.py": '''
from stub_log import append


class Signer:
    def __init__(self, private_key, chain_id=137):
        self.private_key = private_key
        self.chain_id = chain_id
        append({"event": "signer_init", "private_key": private_key, "chain_id": chain_id})

    def address(self):
        return "0xpoly-address"
''',
            "py_clob_client/clob_types.py": '''
from dataclasses import dataclass


@dataclass
class ApiCreds:
    api_key: str
    api_secret: str
    api_passphrase: str


@dataclass
class RequestArgs:
    method: str
    request_path: str
    body: str
''',
            "py_clob_client/headers/__init__.py": "",
            "py_clob_client/headers/headers.py": '''
from stub_log import append


def create_level_2_headers(signer, creds, request_args):
    append({
        "event": "create_level_2_headers",
        "api_key": creds.api_key,
        "api_secret": creds.api_secret,
        "passphrase": creds.api_passphrase,
        "method": request_args.method,
        "request_path": request_args.request_path,
        "body": request_args.body,
        "signer_address": signer.address(),
    })
    return {
        "POLY_ADDRESS": signer.address(),
        "POLY_API_KEY": creds.api_key,
        "POLY_PASSPHRASE": creds.api_passphrase,
        "POLY_SIGNATURE": f"l2sig:{request_args.method}:{request_args.request_path}",
        "POLY_TIMESTAMP": "1712345678",
    }
''',
            "py_order_utils/__init__.py": "",
            "py_order_utils/model.py": '''
from dataclasses import dataclass

EOA = 0
POLY_PROXY = 1
POLY_GNOSIS_SAFE = 2
BUY = "BUY"
SELL = "SELL"


@dataclass
class OrderData:
    maker: str
    taker: str
    tokenId: int
    makerAmount: int
    takerAmount: int
    side: str
    feeRateBps: int
    nonce: int
    signer: str
    expiration: int
    signatureType: int
''',
            "py_order_utils/builders/__init__.py": "",
            "py_order_utils/builders/order_builder.py": '''
from stub_log import append


class SignedOrder:
    def __init__(self, data):
        self.signature = f"ordersig:{data.tokenId}:{data.signatureType}"
        self.salt = str(data.nonce)
        self.maker = data.maker
        self.signer = data.signer

    def dict(self):
        return {
            "signature": self.signature,
            "salt": self.salt,
            "maker": self.maker,
            "signer": self.signer,
        }


class OrderBuilder:
    def __init__(self, signer, sig_type, funder, contract_config):
        append({
            "event": "order_builder_init",
            "sig_type": sig_type,
            "funder": funder,
            "exchange": getattr(contract_config, "exchange", None),
        })
        self.signer = signer
        self.sig_type = sig_type
        self.funder = funder
        self.contract_config = contract_config

    def build_signed_order(self, order_data):
        append({
            "event": "build_signed_order",
            "maker": order_data.maker,
            "signer": order_data.signer,
            "token_id": order_data.tokenId,
            "maker_amount": order_data.makerAmount,
            "taker_amount": order_data.takerAmount,
            "signature_type": order_data.signatureType,
            "side": order_data.side,
        })
        return SignedOrder(order_data)
''',
        }
        for relative_path, contents in files.items():
            path = stub_root / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(textwrap.dedent(contents).lstrip(), encoding="utf-8")

    def test_sign_order_uses_repo_local_sdk_contract_and_emits_signature_and_salt(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            stub_root = Path(tmpdir) / "stubs"
            log_path = Path(tmpdir) / "stub.log"
            self.write_stub_sdk(stub_root)

            env = {
                **BASE_ENV,
                "PYTHONPATH": str(stub_root),
                "STUB_LOG_PATH": str(log_path),
            }
            result = run_script("sign_order.py", ORDER_PAYLOAD, env)

            self.assertEqual(result.returncode, 0, msg=result.stderr)
            output = json.loads(result.stdout)
            self.assertEqual(output["signature"], "ordersig:12345:2")
            self.assertEqual(output["salt"], "7")
            self.assertEqual(output["order"]["maker"], "0xfunder-address")
            self.assertEqual(output["order"]["signer"], "0xpoly-address")

            events = read_log(log_path)
            self.assertIn(
                {
                    "event": "order_builder_init",
                    "exchange": "0xexchange",
                    "funder": "0xfunder-address",
                    "sig_type": 2,
                },
                events,
            )
            self.assertIn(
                {
                    "event": "build_signed_order",
                    "maker": "0xfunder-address",
                    "maker_amount": 1000000,
                    "side": "BUY",
                    "signature_type": 2,
                    "signer": "0xpoly-address",
                    "taker_amount": 2000000,
                    "token_id": 12345,
                },
                events,
            )

    def test_sign_l2_uses_sdk_headers_and_emits_rust_compatible_fields(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            stub_root = Path(tmpdir) / "stubs"
            log_path = Path(tmpdir) / "stub.log"
            self.write_stub_sdk(stub_root)

            env = {
                **BASE_ENV,
                "PYTHONPATH": str(stub_root),
                "STUB_LOG_PATH": str(log_path),
            }
            result = run_script("sign_l2.py", L2_PAYLOAD, env)

            self.assertEqual(result.returncode, 0, msg=result.stderr)
            output = json.loads(result.stdout)
            self.assertEqual(output["signature"], "l2sig:POST:/orders")
            self.assertEqual(output["timestamp"], "1712345678")
            self.assertEqual(output["POLY_SIGNATURE"], "l2sig:POST:/orders")
            self.assertEqual(output["POLY_TIMESTAMP"], "1712345678")

            events = read_log(log_path)
            self.assertIn(
                {
                    "event": "create_level_2_headers",
                    "api_key": "api-key",
                    "api_secret": "api-secret",
                    "body": '{"owner":"owner-uuid"}',
                    "method": "POST",
                    "passphrase": "passphrase",
                    "request_path": "/orders",
                    "signer_address": "0xpoly-address",
                },
                events,
            )

    def test_sign_order_fails_closed_when_sdk_is_unavailable(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            stub_root = Path(tmpdir) / "stubs"
            (stub_root / "py_clob_client").mkdir(parents=True, exist_ok=True)
            (stub_root / "py_clob_client" / "__init__.py").write_text(
                'raise ImportError("py-clob-client unavailable")\n',
                encoding="utf-8",
            )
            env = {
                **BASE_ENV,
                "PYTHONPATH": str(stub_root),
            }
            result = run_script("sign_order.py", ORDER_PAYLOAD, env)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("py-clob-client", result.stderr)


if __name__ == "__main__":
    unittest.main()
