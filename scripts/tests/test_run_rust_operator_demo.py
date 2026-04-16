import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_rust_operator_demo.sh"


BASE_ENV = {
    "POLY_ADDRESS": "0xpoly-address",
    "CLOB_API_KEY": "api-key",
    "CLOB_SECRET": "api-secret",
    "CLOB_PASS_PHRASE": "passphrase",
    "PRIVATE_KEY": "private-key",
    "SIGNATURE_TYPE": "2",
    "FUNDER_ADDRESS": "0xfunder-address",
}


class RunRustOperatorDemoScriptTest(unittest.TestCase):
    def write_stub_cargo(self, root: Path) -> Path:
        stub_path = root / "stub_cargo.py"
        stub_path.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import sys
                sys.stdout.write("mode=operator-demo\\n")
                sys.stdout.write("mode=runtime-smoke\\n")
                sys.stdout.write("helper_smoke=ok\\n")
                sys.stdout.write("leaderboard_hint=cd rust-copytrader && cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20\\n")
                sys.stdout.write("leaderboard_preview_url=https://data-api.polymarket.com/v1/leaderboard?category=OVERALL&timePeriod=DAY&orderBy=PNL&limit=20&offset=0\\n")
                sys.stdout.write("activity_preview_url=https://data-api.polymarket.com/activity?user=0xpoly-address&limit=20&offset=0&sortBy=TIMESTAMP&sortDirection=DESC&type=TRADE\\n")
                sys.stdout.write("leaderboard_capture_hint=cd rust-copytrader && cargo run --bin fetch_trader_leaderboard -- --category OVERALL --time-period DAY --order-by PNL --limit 20 --output ../.omx/discovery/leaderboard-overall-day-pnl.json\\n")
                sys.stdout.write("activity_capture_hint=cd rust-copytrader && cargo run --bin fetch_user_activity -- --user 0xpoly-address --type TRADE --limit 20 --output ../.omx/discovery/activity-0xpoly-address-trade.json\\n")
                sys.stdout.write("leader_selection_hint=cd rust-copytrader && cargo run --bin select_copy_leader -- --leaderboard ../.omx/discovery/leaderboard-overall-day-pnl.json --output ../.omx/discovery/selected-leader.env\\n")
                sys.stdout.write("activity_selection_hint=cd rust-copytrader && cargo run --bin select_copy_leader -- --activity ../.omx/discovery/activity-0xpoly-address-trade.json --output ../.omx/discovery/selected-leader.env\\n")
                sys.stdout.write("discover_copy_leader_hint=cd rust-copytrader && cargo run --bin discover_copy_leader -- --discovery-dir ../.omx/discovery\\n")
                sys.stdout.write("run_copytrader_operator_flow_hint=cd rust-copytrader && cargo run --bin run_copytrader_operator_flow -- --root .. --discovery-dir ../.omx/discovery\\n")
                sys.stdout.write("watch_copy_leader_activity_hint=cd rust-copytrader && cargo run --bin watch_copy_leader_activity -- --root .. --proxy http://127.0.0.1:7897 --poll-count 1\\n")
                sys.stdout.write("run_copytrader_guarded_cycle_hint=cd rust-copytrader && cargo run --bin run_copytrader_guarded_cycle -- --root ..\\n")
                sys.stdout.write("run_copytrader_auto_guarded_loop_hint=cd rust-copytrader && cargo run --bin run_copytrader_auto_guarded_loop -- --root .. --proxy http://127.0.0.1:7897 --watch-poll-count 1 --loop-count 1\\n")
                sys.stdout.write("leader_selection_source_hint=set -a && source .omx/discovery/selected-leader.env && set +a\\n")
                """
            ),
            encoding="utf-8",
        )
        stub_path.chmod(0o755)
        return stub_path

    def test_run_rust_operator_demo_executes_cargo_operator_demo(self):
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
            self.assertIn("mode=operator-demo", result.stdout)
            self.assertIn("mode=runtime-smoke", result.stdout)
            self.assertIn("helper_smoke=ok", result.stdout)
            self.assertIn("leaderboard_hint=", result.stdout)
            self.assertIn("leaderboard_preview_url=", result.stdout)
            self.assertIn("activity_preview_url=", result.stdout)
            self.assertIn("leaderboard_capture_hint=", result.stdout)
            self.assertIn("activity_capture_hint=", result.stdout)
            self.assertIn("leader_selection_hint=", result.stdout)
            self.assertIn("activity_selection_hint=", result.stdout)
            self.assertIn("discover_copy_leader_hint=", result.stdout)
            self.assertIn("run_copytrader_operator_flow_hint=", result.stdout)
            self.assertIn("watch_copy_leader_activity_hint=", result.stdout)
            self.assertIn("run_copytrader_guarded_cycle_hint=", result.stdout)
            self.assertIn("run_copytrader_auto_guarded_loop_hint=", result.stdout)
            self.assertIn("leader_selection_source_hint=", result.stdout)

    def test_run_rust_operator_demo_fails_closed_without_required_env(self):
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
