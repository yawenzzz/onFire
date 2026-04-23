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
        self.assertIn('args=(--root "$ROOT")', text)
        self.assertIn('args=(--root "$ROOT" "${args[@]}")', text)
        self.assertIn('"$CARGO_BIN" run --bin run_copytrader_fill_latency_logger -- "${args[@]}"', text)

    def test_live_submit_latency_wrapper_runs_logger_only_and_uses_logs_dir(self) -> None:
        text = Path("scripts/run_rust_minmax_follow_live_submit_latency.sh").read_text()
        self.assertIn('LATENCY_LOGGER_BIN="${LATENCY_LOGGER_BIN:-$ROOT/scripts/run_rust_copytrade_fill_latency_logger.sh}"', text)
        self.assertIn('LOG_ROOT="${LOG_ROOT:-$ROOT/logs/copytrade-fill-latency/$LEADER_KEY}"', text)
        self.assertIn('echo "[info]: fill latency logger only"', text)
        self.assertIn('echo "[info]: log_file=$LOG_ROOT/fills.log"', text)
        self.assertIn('cmd=(bash "$LATENCY_LOGGER_BIN" --user "$USER_WALLET" --log-dir "$LOG_ROOT")', text)
        self.assertIn('exec "${cmd[@]}"', text)
        self.assertNotIn('FOLLOW_BIN="${FOLLOW_BIN', text)
        self.assertNotIn('follow.stdout.log', text)

    def test_live_submit_latency_wrapper_uses_stubbed_logger_only(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="fill-latency-wrapper-") as tmpdir:
            tmp = Path(tmpdir)
            logger = tmp / "logger.sh"
            self._make_exec(
                logger,
                "#!/usr/bin/env bash\nprintf '[info]: latency_ms=123 fill_ts_source=matchtime corr=order_id price_gap_bps=10.0000 shares=5.00 leader_tx=0xabc trade_id=trade-1\\n'\n",
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
                    "LATENCY_LOGGER_BIN": str(logger),
                    "LOG_ROOT": str(tmp / "latency"),
                },
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("[info]: fill latency logger only", completed.stdout)
            self.assertIn("[info]: log_file=", completed.stdout)
            self.assertIn("run main follow separately", completed.stdout)
            self.assertIn("[info]: latency_ms=123", completed.stdout)
            self.assertFalse((tmp / "latency" / "follow.stdout.log").exists())
