import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


WALLET = "0x11084005d88A0840b5F38F8731CCa9152BbD99F7"


class CopytradeFillLatencyScriptTests(unittest.TestCase):
    def _make_exec(self, path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_fill_latency_logger_wrapper_targets_rust_bin(self) -> None:
        text = Path("scripts/run_rust_copytrade_fill_latency_logger.sh").read_text()
        self.assertIn("run_copytrader_fill_latency_logger", text)
        self.assertIn('"$CARGO_BIN" run --bin run_copytrader_fill_latency_logger -- "$@"', text)

    def test_live_submit_latency_wrapper_starts_logger_and_redirects_follow_logs(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_submit_latency.sh").read_text()
        self.assertIn('FOLLOW_BIN="${FOLLOW_BIN:-$ROOT/scripts/run_rust_minmax_follow_live_submit.sh}"', text)
        self.assertIn('FILL_LATENCY_LOGGER_BIN="${FILL_LATENCY_LOGGER_BIN:-$ROOT/scripts/run_rust_copytrade_fill_latency_logger.sh}"', text)
        self.assertIn('bash "$FILL_LATENCY_LOGGER_BIN" --user "$USER_WALLET" --log-dir "$LOG_ROOT" &', text)
        self.assertIn('bash "$FOLLOW_BIN" "${ARGS[@]}" >>"$FOLLOW_STDOUT_LOG" 2>>"$FOLLOW_STDERR_LOG"', text)

    def test_live_submit_latency_wrapper_uses_stubbed_logger_and_follow_bins(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="fill-latency-wrapper-") as tmpdir:
            tmp = Path(tmpdir)
            logger = tmp / "logger.sh"
            follow = tmp / "follow.sh"
            self._make_exec(
                logger,
                "#!/usr/bin/env bash\nprintf 'fill latency_ms=123 price_gap_bps=10.0000 shares=5.00 leader_tx=0xabc trade_id=trade-1\\n'\nsleep 1\n",
            )
            self._make_exec(
                follow,
                "#!/usr/bin/env bash\nprintf 'follow-stdout\\n'\nprintf 'follow-stderr\\n' >&2\n",
            )

            completed = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_minmax_follow_live_submit_latency.sh",
                    "--user",
                    WALLET,
                ],
                cwd=root,
                env={
                    **os.environ,
                    "FOLLOW_BIN": str(follow),
                    "FILL_LATENCY_LOGGER_BIN": str(logger),
                    "LOG_ROOT": str(tmp / "latency"),
                },
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("live_submit_latency_ready", completed.stdout)
            self.assertIn("fill latency_ms=123", completed.stdout)
            self.assertTrue((tmp / "latency" / "follow.stdout.log").exists())
            self.assertTrue((tmp / "latency" / "follow.stderr.log").exists())
            self.assertIn("follow-stdout", (tmp / "latency" / "follow.stdout.log").read_text())
            self.assertIn("follow-stderr", (tmp / "latency" / "follow.stderr.log").read_text())
