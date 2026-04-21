import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


class AccountMonitorScriptTests(unittest.TestCase):
    def test_show_account_info_wrapper_invokes_rust_bin(self) -> None:
        text = Path("scripts/run_rust_show_account_info.sh").read_text()
        self.assertIn("run_copytrader_account_monitor", text)
        self.assertIn("--json", text)
        self.assertIn('--root "$ROOT"', text)
        self.assertIn('if [[ $# -eq 0 ]]; then', text)
        self.assertIn('elif [[ " ${args[*]-} " != *" --root "* ]]; then', text)

    def test_account_monitor_wrapper_invokes_rust_bin_in_watch_mode(self) -> None:
        text = Path("scripts/run_rust_account_monitor.sh").read_text()
        self.assertIn("run_copytrader_account_monitor", text)
        self.assertIn("--watch", text)
        self.assertIn("--interval-secs", text)
        self.assertIn("--output", text)
        self.assertIn('if [[ $# -eq 0 ]]; then', text)
        self.assertIn('elif [[ " ${args[*]-} " != *" --root "* ]]; then', text)

    def test_account_user_ws_wrapper_invokes_rust_ws_bin(self) -> None:
        text = Path("scripts/run_rust_account_user_ws.sh").read_text()
        self.assertIn("run_copytrader_account_ws", text)
        self.assertIn("--json", text)
        self.assertIn('--root "$ROOT"', text)
        self.assertIn('if [[ $# -eq 0 ]]; then', text)
        self.assertIn('elif [[ " ${args[*]-} " != *" --root "* ]]; then', text)

    def _make_exec(self, path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_show_account_info_wrapper_e2e_injects_default_root(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="acct-show-wrapper-") as tmpdir:
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
                ["bash", "scripts/run_rust_show_account_info.sh"],
                cwd=root,
                env={**os.environ, "CARGO_BIN": str(stub)},
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("stub-cargo-called", result.stdout)
            forwarded = log.read_text()
            self.assertIn("run", forwarded)
            self.assertIn("run_copytrader_account_monitor", forwarded)
            self.assertIn("--json", forwarded)
            self.assertIn("--root", forwarded)
            self.assertIn(str(root), forwarded)

    def test_account_monitor_wrapper_e2e_forwards_watch_flags(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="acct-monitor-wrapper-") as tmpdir:
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
                ["bash", "scripts/run_rust_account_monitor.sh", "--max-iterations", "1"],
                cwd=root,
                env={
                    **os.environ,
                    "CARGO_BIN": str(stub),
                    "INTERVAL_SECS": "2",
                    "OUTPUT_PATH": ".omx/account-monitor/test.json",
                },
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("stub-cargo-called", result.stdout)
            forwarded = log.read_text()
            self.assertIn("run_copytrader_account_monitor", forwarded)
            self.assertIn("--watch", forwarded)
            self.assertIn("--interval-secs", forwarded)
            self.assertIn("2", forwarded)
            self.assertIn("--output", forwarded)
            self.assertIn(".omx/account-monitor/test.json", forwarded)
            self.assertIn("--max-iterations", forwarded)
            self.assertIn("1", forwarded)

    def test_account_user_ws_wrapper_e2e_no_args_does_not_crash(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="acct-ws-wrapper-") as tmpdir:
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
                ["bash", "scripts/run_rust_account_user_ws.sh"],
                cwd=root,
                env={**os.environ, "CARGO_BIN": str(stub)},
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("stub-cargo-called", result.stdout)
            forwarded = log.read_text()
            self.assertIn("run_copytrader_account_ws", forwarded)
            self.assertIn("--json", forwarded)
            self.assertIn("--root", forwarded)
            self.assertIn(str(root), forwarded)
