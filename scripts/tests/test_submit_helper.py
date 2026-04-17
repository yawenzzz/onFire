import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "submit_helper.py"


class SubmitHelperScriptTest(unittest.TestCase):
    def write_stub_curl(self, root: Path, body: str, exit_code: int = 0) -> tuple[Path, Path]:
        log_path = root / "curl.log"
        stub_path = root / "stub_curl.py"
        stub_path.write_text(
            textwrap.dedent(
                f"""\
                #!/usr/bin/env python3
                import pathlib
                import sys

                pathlib.Path({str(log_path)!r}).write_text("\\n".join(sys.argv[1:]), encoding="utf-8")
                sys.stdout.write({body!r})
                raise SystemExit({exit_code})
                """
            ),
            encoding="utf-8",
        )
        stub_path.chmod(0o755)
        return stub_path, log_path

    def test_submit_helper_forwards_curl_compatible_args_and_output(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            stub_curl, log_path = self.write_stub_curl(
                root,
                '{"ok":true}\n__HTTP_STATUS__:200',
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--json",
                    "--curl-bin",
                    str(stub_curl),
                    "--silent",
                    "--show-error",
                    "-X",
                    "POST",
                    "https://helper.polymarket.test/orders",
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
                env=os.environ.copy(),
            )

            self.assertEqual(result.returncode, 0, msg=result.stderr)
            self.assertEqual(result.stdout, '{"ok":true}\n__HTTP_STATUS__:200')
            forwarded = log_path.read_text(encoding="utf-8")
            self.assertIn("--silent", forwarded)
            self.assertIn("--show-error", forwarded)
            self.assertIn("https://helper.polymarket.test/orders", forwarded)

    def test_submit_helper_fails_closed_without_forwarded_args(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--json"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            env=os.environ.copy(),
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("expected curl-style arguments", result.stderr)


if __name__ == "__main__":
    unittest.main()
