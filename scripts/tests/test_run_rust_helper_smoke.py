import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_rust_helper_smoke.sh"


BASE_ENV = {
    "POLY_ADDRESS": "0xpoly-address",
    "CLOB_API_KEY": "api-key",
    "CLOB_SECRET": "api-secret",
    "CLOB_PASS_PHRASE": "passphrase",
    "PRIVATE_KEY": "private-key",
    "SIGNATURE_TYPE": "2",
    "FUNDER_ADDRESS": "0xfunder-address",
}


class RunRustHelperSmokeScriptTest(unittest.TestCase):
    maxDiff = None

    def write_stub_sdk(self, stub_root: Path) -> None:
        files = {
            "py_clob_client/__init__.py": "",
            "py_clob_client/config.py": """
from types import SimpleNamespace
def get_contract_config(chain_id, neg_risk=False):
    return SimpleNamespace(exchange="0xexchange")
""",
            "py_clob_client/signer.py": """
class Signer:
    def __init__(self, private_key, chain_id=137):
        self.private_key = private_key
        self.chain_id = chain_id
    def address(self):
        return "0xpoly-address"
""",
            "py_clob_client/clob_types.py": """
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
""",
            "py_clob_client/headers/__init__.py": "",
            "py_clob_client/headers/headers.py": """
def create_level_2_headers(signer, creds, request_args):
    return {
        "POLY_ADDRESS": signer.address(),
        "POLY_API_KEY": creds.api_key,
        "POLY_PASSPHRASE": creds.api_passphrase,
        "POLY_SIGNATURE": "l2sig:POST:/orders",
        "POLY_TIMESTAMP": "1712345678",
    }
""",
            "py_order_utils/__init__.py": "",
            "py_order_utils/model.py": """
from dataclasses import dataclass
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
""",
            "py_order_utils/builders/__init__.py": "",
            "py_order_utils/builders/order_builder.py": """
class SignedOrder:
    def __init__(self, data):
        self.signature = "ordersig:12345:2"
        self.salt = "7"
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
        self.signer = signer
    def build_signed_order(self, order_data):
        return SignedOrder(order_data)
""",
        }
        for relative_path, contents in files.items():
            path = stub_root / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(textwrap.dedent(contents).lstrip(), encoding="utf-8")

    def write_stub_cargo(self, root: Path) -> Path:
        stub_path = root / "stub_cargo.py"
        stub_path.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import sys
                sys.stdout.write("requested_mode=live_listen\\n")
                sys.stdout.write("decision=blocked:activity_source_unverified\\n")
                sys.stdout.write("live_mode_unlocked=false\\n")
                sys.stdout.write("signing_command=python3 scripts/sign_order.py --json\\n")
                sys.stdout.write("l2_header_helper=python3 scripts/sign_l2.py --json\\n")
                sys.stdout.write("submit_command=curl\\n")
                """
            ),
            encoding="utf-8",
        )
        stub_path.chmod(0o755)
        return stub_path

    def test_run_rust_helper_smoke_executes_helpers_and_bootstrap_report(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            stub_root = Path(tmpdir) / "stubs"
            self.write_stub_sdk(stub_root)
            cargo_stub = self.write_stub_cargo(Path(tmpdir))

            env = {
                **os.environ,
                **BASE_ENV,
                "PYTHONPATH": str(stub_root),
                "PYTHON_BIN": sys.executable,
                "CARGO_BIN": str(cargo_stub),
            }
            result = subprocess.run(
                ["bash", str(SCRIPT), str(ROOT)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                env=env,
            )

            self.assertEqual(result.returncode, 0, msg=result.stderr)
            self.assertIn("== sign_order.py ==", result.stdout)
            self.assertIn("ordersig:12345:2", result.stdout)
            self.assertIn("== sign_l2.py ==", result.stdout)
            self.assertIn("l2sig:POST:/orders", result.stdout)
            self.assertIn("== rust helper smoke report ==", result.stdout)
            self.assertIn("decision=blocked:activity_source_unverified", result.stdout)

    def test_run_rust_helper_smoke_fails_closed_without_required_env(self):
        result = subprocess.run(
            ["bash", str(SCRIPT), str(ROOT)],
            cwd=ROOT,
            text=True,
            capture_output=True,
            env=os.environ.copy(),
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("missing required env", result.stderr)


if __name__ == "__main__":
    unittest.main()
