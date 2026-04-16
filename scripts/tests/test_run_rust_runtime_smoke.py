import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_rust_runtime_smoke.sh"


BASE_ENV = {
    "POLY_ADDRESS": "0xpoly-address",
    "CLOB_API_KEY": "api-key",
    "CLOB_SECRET": "api-secret",
    "CLOB_PASS_PHRASE": "passphrase",
    "PRIVATE_KEY": "private-key",
    "SIGNATURE_TYPE": "2",
    "FUNDER_ADDRESS": "0xfunder-address",
}


class RunRustRuntimeSmokeScriptTest(unittest.TestCase):
    def write_stub_cargo(self, root: Path) -> Path:
        stub_path = root / "stub_cargo.py"
        stub_path.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import sys
                sys.stdout.write("mode=runtime-smoke\\n")
                sys.stdout.write("helper_smoke=ok\\n")
                sys.stdout.write("session_outcome=processed\\n")
                sys.stdout.write("runtime_mode=replay\\n")
                sys.stdout.write("last_submit_status=verified\\n")
                """
            ),
            encoding="utf-8",
        )
        stub_path.chmod(0o755)
        return stub_path

    def test_run_rust_runtime_smoke_executes_cargo_runtime_smoke(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            cargo_stub = self.write_stub_cargo(Path(tmpdir))
            env = {
                **os.environ,
                **BASE_ENV,
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
            self.assertIn("mode=runtime-smoke", result.stdout)
            self.assertIn("helper_smoke=ok", result.stdout)
            self.assertIn("session_outcome=processed", result.stdout)

    def test_run_rust_runtime_smoke_fails_closed_without_required_env(self):
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
