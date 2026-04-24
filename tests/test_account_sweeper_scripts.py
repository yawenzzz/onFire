import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


class AccountSweeperScriptTests(unittest.TestCase):
    def test_account_sweeper_wrapper_invokes_rust_bin_in_watch_mode(self) -> None:
        text = Path("scripts/run_rust_account_sweeper.sh").read_text()
        self.assertIn("run_copytrader_account_sweeper", text)
        self.assertIn("--watch", text)
        self.assertIn("--interval-secs", text)
        self.assertIn('ALLOW_LIVE_SUBMIT="${ALLOW_LIVE_SUBMIT:-1}"', text)
        self.assertIn('tee -a "$LOG_FILE"', text)
        self.assertIn('independent_of_main_follow=true', text)
        self.assertIn('elif [[ " ${args[*]-} " != *" --root "* ]]; then', text)

    def _make_exec(self, path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_account_sweeper_wrapper_defaults_to_live_submit_and_root(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="acct-sweeper-wrapper-") as tmpdir:
            stub = Path(tmpdir) / "cargo"
            log = Path(tmpdir) / "args.txt"
            self._make_exec(
                stub,
                f"""#!/usr/bin/env bash
printf '%s\\n' "$@" > "{log}"
printf 'stub-cargo-called\\n'
""",
            )
            result = subprocess.run(
                ["bash", "scripts/run_rust_account_sweeper.sh", "--max-iterations", "1"],
                cwd=root,
                env={
                    **os.environ,
                    "CARGO_BIN": str(stub),
                    "LOG_FILE": str(Path(tmpdir) / "sweeper.log"),
                },
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("stub-cargo-called", result.stdout)
            self.assertIn("[info]: account sweeper independent loop", result.stdout)
            forwarded = log.read_text()
            self.assertIn("run_copytrader_account_sweeper", forwarded)
            self.assertIn("--watch", forwarded)
            self.assertIn("--interval-secs", forwarded)
            self.assertIn("--root", forwarded)
            self.assertIn(str(root), forwarded)
            self.assertIn("--allow-live-submit", forwarded)
            self.assertIn("--max-iterations", forwarded)
            self.assertIn("1", forwarded)

    def test_account_sweeper_wrapper_can_run_preview_mode(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="acct-sweeper-preview-") as tmpdir:
            stub = Path(tmpdir) / "cargo"
            log = Path(tmpdir) / "args.txt"
            self._make_exec(
                stub,
                f"""#!/usr/bin/env bash
printf '%s\\n' "$@" > "{log}"
printf 'stub-cargo-called\\n'
""",
            )
            result = subprocess.run(
                ["bash", "scripts/run_rust_account_sweeper.sh", "--max-iterations", "1"],
                cwd=root,
                env={
                    **os.environ,
                    "CARGO_BIN": str(stub),
                    "ALLOW_LIVE_SUBMIT": "0",
                    "LOG_FILE": str(Path(tmpdir) / "sweeper.log"),
                },
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("[info]: mode=preview", result.stdout)
            forwarded = log.read_text()
            self.assertNotIn("--allow-live-submit", forwarded)
            self.assertIn("--watch", forwarded)
