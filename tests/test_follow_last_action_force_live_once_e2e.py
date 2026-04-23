import os
import shutil
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


WALLET = "0x11084005d88A0840b5F38F8731CCa9152BbD99F7"


class FollowLastActionForceLiveOnceE2ETests(unittest.TestCase):
    def _make_exec(self, path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def _write_non_exec(self, path: Path, content: str) -> None:
        path.write_text(content)

    def _make_positions_gate_exec(
        self,
        path: Path,
        *,
        status: str = "follow_new_open",
        reason: str = "current_position_matches_latest_trade_size",
        should_follow: bool = True,
        target_size: str = "60.000000",
        other_size: str = "0.000000",
        total_size: str | None = None,
        response_count: str = "1",
        event_count: str = "1",
    ) -> None:
        if total_size is None:
            total_size = target_size
        self._make_exec(
            path,
            "\n".join(
                [
                    "#!/usr/bin/env bash",
                    "printf 'mode=public-positions-gate\\n'",
                    "printf 'positions_query_status=ok\\n'",
                    "printf 'positions_retry_attempts=1\\n'",
                    f"printf 'current_positions_response_count={response_count}\\n'",
                    f"printf 'current_event_position_count={event_count}\\n'",
                    f"printf 'current_event_target_asset_size={target_size}\\n'",
                    f"printf 'current_event_other_asset_size={other_size}\\n'",
                    f"printf 'current_event_total_size={total_size}\\n'",
                    f"printf 'leader_event_open_gate_status={status}\\n'",
                    f"printf 'leader_event_open_gate_reason={reason}\\n'",
                    f"printf 'leader_event_should_follow={'true' if should_follow else 'false'}\\n'",
                ]
            )
            + "\n",
        )

    def _make_account_snapshot_exec(self, path: Path, snapshot_json: str) -> None:
        self._make_exec(
            path,
            "\n".join(
                [
                    "#!/usr/bin/env bash",
                    'out=""',
                    'while [ $# -gt 0 ]; do',
                    '  if [ "$1" = "--output" ]; then out="$2"; shift 2; continue; fi',
                    "  shift",
                    "done",
                    'if [ -z "$out" ]; then exit 2; fi',
                    'mkdir -p "$(dirname "$out")"',
                    "cat > \"$out\" <<'EOF_SNAPSHOT'",
                    snapshot_json,
                    "EOF_SNAPSHOT",
                ]
            )
            + "\n",
        )

    def test_force_live_once_uses_wrapper_bins_and_force_flags(self) -> None:
        wallet = WALLET.lower()
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / wallet
        activity_root = root / ".omx" / "live-activity" / wallet
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-test-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":60.0,"usdcSize":30.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % wallet
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\n'\n" % wallet,
            )
            self._make_positions_gate_exec(positions_gate)
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\nprintf 'leader_price=0.50000000\\n'\nprintf 'follower_effective_price=0.50000000\\n'\nprintf 'price_gap_bps=0.0000\\n'\n"
                % ("%s", submit_log),
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""
            env["IGNORE_SEEN_TX"] = "1"
            env["REQUIRE_NEW_ACTIVITY"] = "0"

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", wallet],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(
                completed.returncode,
                0,
                f"stdout:\n{completed.stdout}\n\nstderr:\n{completed.stderr}",
            )

            stdout = completed.stdout
            self.assertIn("rust follow last action force live once", stdout)
            self.assertIn("cargo=cargo", stdout)
            self.assertIn("run_dir=", stdout)
            forwarded = submit_log.read_text()
            self.assertIn("--allow-live-submit", forwarded)
            self.assertIn("--force-live-submit", forwarded)
            self.assertIn("--override-usdc-size", forwarded)
            self.assertIn("3.000000", forwarded)
            self.assertIn("--order-type", forwarded)
            self.assertIn("GTC", forwarded)
            runs_dir = root / ".omx" / "force-live-follow" / wallet / "runs"
            self.assertTrue(runs_dir.exists())
            run_dirs = list(runs_dir.iterdir())
            self.assertEqual(len(run_dirs), 1)
            self.assertTrue((run_dirs[0] / "watch.stdout.log").exists())
            self.assertTrue((run_dirs[0] / "submit.stdout.log").exists())
            summary = (run_dirs[0] / "summary.txt").read_text()
            self.assertIn("watch_exit=0", summary)
            self.assertIn("submit_exit=0", summary)
            self.assertIn("status=submit_completed", summary)
            self.assertEqual(summary.count("price_gap_bps=0.0000"), 1)
            latest_run = (root / ".omx" / "force-live-follow" / wallet / "latest-run.txt").read_text().strip()
            self.assertEqual(Path(latest_run), run_dirs[0])
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_skips_duplicate_latest_tx_by_default(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-dup-test-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)
            state_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xold","price":0.1,"side":"BUY","slug":"market-a"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xdup","price":0.9,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET)
            )
            (state_root / "last-submitted-tx.txt").write_text("0xdup\n")

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xdup\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_already_holds_other_outcome_in_event",
                should_follow=False,
                target_size="60.000000",
                other_size="20.000000",
                total_size="80.000000",
                response_count="2",
                event_count="2",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-yes","net_size":"12.0","last_price":"0.5","estimated_equity":"6.0"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("skipping duplicate latest tx: 0xdup", completed.stdout)
            self.assertFalse(submit_log.exists())
            runs_dir = root / ".omx" / "force-live-follow" / WALLET / "runs"
            run_dirs = list(runs_dir.iterdir())
            self.assertEqual(len(run_dirs), 1)
            summary = (run_dirs[0] / "summary.txt").read_text()
            self.assertIn("status=duplicate_tx_skipped", summary)
            self.assertIn("submit_exit=not_run", summary)
            self.assertIn("latest_tx=0xdup", summary)
            self.assertIn("latest_activity_timestamp=20", summary)
            self.assertIn("latest_activity_price=0.9", summary)
            self.assertIn("watch_started_at_unix_ms=", summary)
            self.assertIn("watch_finished_at_unix_ms=", summary)
            self.assertIn("watch_elapsed_ms=", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_can_skip_when_require_new_activity_is_enabled(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-no-new-activity-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xstale","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=0\\n'\n" % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_already_holds_other_outcome_in_event",
                should_follow=False,
                target_size="60.000000",
                other_size="20.000000",
                total_size="80.000000",
                response_count="2",
                event_count="2",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-yes","net_size":"12.0","last_price":"0.5","estimated_equity":"6.0"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""
            env["REQUIRE_NEW_ACTIVITY"] = "1"

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("skipping because watch reported no new activity", completed.stdout)
            self.assertFalse(submit_log.exists())
            runs_dir = root / ".omx" / "force-live-follow" / WALLET / "runs"
            run_dirs = list(runs_dir.iterdir())
            self.assertEqual(len(run_dirs), 1)
            summary = (run_dirs[0] / "summary.txt").read_text()
            self.assertIn("status=no_new_activity_skipped", summary)
            self.assertIn("latest_tx=0xstale", summary)
            self.assertIn("watch_elapsed_ms=", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_skips_existing_event_position_before_new_buy(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-existing-event-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-no","conditionId":"cond-1","outcome":"No","size":20.0,"usdcSize":10.0,"transactionHash":"0xold","price":0.5,"side":"BUY","slug":"market-a"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-yes","conditionId":"cond-1","outcome":"Yes","size":60.0,"usdcSize":30.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET)
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_already_holds_other_outcome_in_event",
                should_follow=False,
                target_size="60.000000",
                other_size="20.000000",
                total_size="80.000000",
                response_count="2",
                event_count="2",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-yes","net_size":"12.0","last_price":"0.5","estimated_equity":"6.0"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertTrue(submit_log.exists())
            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary = (run_dir / "summary.txt").read_text()
            self.assertIn("status=submit_completed", summary)
            self.assertIn("follower_current_asset_held=true", summary)
            self.assertIn("follow_trigger_reason=follower_holds_asset", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_floors_first_open_to_min_shares_when_scaled_follow_shares_too_small(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-share-floor-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-small","size":40.0,"usdcSize":20.0,"transactionHash":"0xsmall","price":0.5,"side":"BUY","slug":"market-small"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xsmall\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                target_size="40.000000",
                total_size="40.000000",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("live_submit_status=submitted", completed.stdout)
            self.assertTrue(submit_log.exists())
            forwarded = submit_log.read_text()
            self.assertIn("--override-usdc-size", forwarded)
            self.assertIn("2.500000", forwarded)
            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary = (run_dir / "summary.txt").read_text()
            self.assertIn("status=submit_completed", summary)
            self.assertIn("follow_share_size=5", summary)
            self.assertIn("follow_min_open_floor_applied=true", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_accepts_follow_share_divisor_override(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-divisor-override-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":120.0,"usdcSize":60.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(positions_gate)
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_follow_last_action_force_live_once.sh",
                    "--user",
                    WALLET,
                    "--follow-share-divisor",
                    "20",
                ],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("follow_share_divisor=20", completed.stdout)
            forwarded = submit_log.read_text()
            self.assertIn("3.000000", forwarded)
            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary = (run_dir / "summary.txt").read_text()
            self.assertIn("follow_share_divisor=20", summary)
            self.assertIn("follow_share_size=6.000000", summary)
            self.assertIn("follow_min_open_floor_applied=false", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_allows_add_on_below_five_shares_when_follower_already_holds_asset(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-add-on-small-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":40.0,"usdcSize":20.0,"transactionHash":"0xadd","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xadd\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_already_holds_other_outcome_in_event",
                should_follow=False,
                target_size="40.000000",
                other_size="10.000000",
                total_size="50.000000",
                response_count="2",
                event_count="2",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-1","net_size":"8.0","last_price":"0.5","estimated_equity":"4.0"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("live_submit_status=submitted", completed.stdout)
            forwarded = submit_log.read_text()
            self.assertIn("2.000000", forwarded)
            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary = (run_dir / "summary.txt").read_text()
            self.assertIn("follow_share_size=4.000000", summary)
            self.assertIn("follow_min_open_floor_applied=false", summary)
            self.assertIn("follow_min_compatible_floor_applied=false", summary)
            self.assertIn("follow_trigger_reason=follower_holds_asset", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_floors_tiny_add_on_to_min_compatible_share(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-add-on-compatible-floor-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":0.02,"usdcSize":0.01,"transactionHash":"0xtiny","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xtiny\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_already_holds_other_outcome_in_event",
                should_follow=False,
                target_size="0.020000",
                other_size="10.000000",
                total_size="10.020000",
                response_count="2",
                event_count="2",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-1","net_size":"1.0","last_price":"0.5","estimated_equity":"0.5"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("live_submit_status=submitted", completed.stdout)
            forwarded = submit_log.read_text()
            self.assertIn("0.005000", forwarded)
            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary = (run_dir / "summary.txt").read_text()
            self.assertIn("follow_share_size=0.01", summary)
            self.assertIn("follow_min_open_floor_applied=false", summary)
            self.assertIn("follow_min_compatible_floor_applied=true", summary)
            self.assertIn("follow_trigger_reason=follower_holds_asset", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_prefers_watch_latest_new_tx_over_first_json_tx(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-watch-tx-test-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-old","conditionId":"cond-old","size":20.0,"usdcSize":10.0,"transactionHash":"0xold","price":0.5,"side":"BUY","slug":"market-b"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-new","size":60.0,"usdcSize":30.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET)
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"
            submit_activity = log_dir / "submit-latest-activity.json"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nlatest=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--latest-activity\" ]; then latest=\"$2\"; shift 2; continue; fi\n  shift\n done\ncat \"$latest\" > %s\nprintf 'live_submit_status=submitted\\n'\nprintf 'leader_price=0.50000000\\n'\nprintf 'follower_effective_price=0.50000000\\n'\nprintf 'price_gap_bps=0.0000\\n'\n"
                % ("%s", submit_log, submit_activity),
            )
            self._make_positions_gate_exec(positions_gate)
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            last_submitted = (
                root / ".omx" / "force-live-follow" / WALLET / "last-submitted-tx.txt"
            ).read_text().strip()
            self.assertEqual(last_submitted, "0xnew")
            selected_payload = submit_activity.read_text()
            self.assertIn("0xnew", selected_payload)
            self.assertNotIn("0xold", selected_payload)

            run_dir = next((root / ".omx" / "force-live-follow" / WALLET / "runs").iterdir())
            summary_path = run_dir / "summary.txt"
            summary = summary_path.read_text()
            self.assertIn("latest_activity_timestamp=20", summary)
            self.assertIn("latest_activity_price=0.5", summary)
            self.assertIn("selected_latest_activity=", summary)

            latency = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_copytrade_latency_report.sh",
                    "--report",
                    str(summary_path),
                ],
                cwd=root,
                capture_output=True,
                text=True,
                check=True,
            )
            self.assertIn("leader_timestamp=20", latency.stdout)
            self.assertIn("leader_price=0.5", latency.stdout)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_falls_back_to_latest_timestamp_when_latest_new_tx_missing(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-fallback-tx-test-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-old","conditionId":"cond-old","size":20.0,"usdcSize":2.0,"transactionHash":"0xold","price":0.1,"side":"BUY","slug":"market-b"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-new","conditionId":"cond-new","size":60.0,"usdcSize":54.0,"transactionHash":"0xnew","price":0.9,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET)
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_activity = log_dir / "submit-latest-activity.json"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nlatest=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--latest-activity\" ]; then latest=\"$2\"; shift 2; continue; fi\n  shift\n done\ncat \"$latest\" > %s\nprintf 'live_submit_status=submitted\\n'\nprintf 'leader_price=0.90000000\\n'\nprintf 'follower_effective_price=0.90000000\\n'\nprintf 'price_gap_bps=0.0000\\n'\n"
                % submit_activity,
            )
            self._make_positions_gate_exec(
                positions_gate,
                target_size="60.000000",
                total_size="60.000000",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            selected_payload = submit_activity.read_text()
            self.assertIn("0xnew", selected_payload)
            self.assertNotIn("0xold", selected_payload)
            self.assertIn("asset-new", selected_payload)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_does_not_mark_seen_when_submit_success_false(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-submit-false-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":60.0,"usdcSize":30.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )
            (activity_root / "seen-tx.txt").write_text("0xnew\n")

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xnew\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf 'live_submit_status=submitted\\nsubmit_success=false\\n'\n",
            )
            self._make_positions_gate_exec(positions_gate)
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[{"asset_id":"asset-1","net_size":"15.0","last_price":"0.5","estimated_equity":"7.5"}],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertFalse((state_root / "last-submitted-tx.txt").exists())
            seen_after = (activity_root / "seen-tx.txt").read_text()
            self.assertNotIn("0xnew", seen_after)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_routes_merge_activity_to_ctf_action_bin(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-merge-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"MERGE","conditionId":"0x1111111111111111111111111111111111111111111111111111111111111111","usdcSize":1.5,"transactionHash":"0xmerge","outcome":"No","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"
            ctf = log_dir / "ctf.sh"
            ctf_args = log_dir / "ctf-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xmerge\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf 'called\\n' >> %s\nprintf 'live_submit_status=submitted\\n'\n"
                % submit_log,
            )
            self._make_exec(
                ctf,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'ctf_action_type=MERGE\\n'\nprintf 'ctf_action_status=submitted\\n'\n"
                % ("%s", ctf_args),
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["CTF_ACTION_BIN_DEFAULT"] = str(ctf)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("ctf_action_type=MERGE", completed.stdout)
            self.assertFalse(submit_log.exists())
            ctf_forwarded = ctf_args.read_text()
            self.assertIn("--latest-activity", ctf_forwarded)
            self.assertIn("--allow-live-submit", ctf_forwarded)
            runs_dir = root / ".omx" / "force-live-follow" / WALLET / "runs"
            run_dirs = list(runs_dir.iterdir())
            self.assertEqual(len(run_dirs), 1)
            summary = (run_dirs[0] / "summary.txt").read_text()
            self.assertIn("latest_activity_type=MERGE", summary)
            self.assertIn("ctf_action_type=MERGE", summary)
            self.assertIn("ctf_action_status=submitted", summary)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_can_invoke_non_executable_ctf_shell_wrapper(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-merge-noexec-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"MERGE","conditionId":"0x1111111111111111111111111111111111111111111111111111111111111111","usdcSize":1.5,"transactionHash":"0xmerge","outcome":"No","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            ctf = log_dir / "ctf.sh"
            ctf_args = log_dir / "ctf-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xmerge\\n'\n"
                % WALLET,
            )
            self._write_non_exec(
                ctf,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'ctf_action_type=MERGE\\n'\nprintf 'ctf_action_status=submitted\\n'\n"
                % ("%s", ctf_args),
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["CTF_ACTION_BIN_DEFAULT"] = str(ctf)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("ctf_action_type=MERGE", completed.stdout)
            self.assertTrue(ctf_args.exists())
            forwarded = ctf_args.read_text()
            self.assertIn("--latest-activity", forwarded)
            self.assertIn("--allow-live-submit", forwarded)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_processes_every_new_trade_in_same_poll_burst(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-multi-burst-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":30,"type":"TRADE","asset":"asset-yes","conditionId":"cond-burst","outcome":"Yes","size":30.0,"usdcSize":15.0,"transactionHash":"0xtx3","price":0.5,"side":"BUY","slug":"market-a"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-yes","conditionId":"cond-burst","outcome":"Yes","size":20.0,"usdcSize":10.0,"transactionHash":"0xtx2","price":0.5,"side":"BUY","slug":"market-a"},{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-yes","conditionId":"cond-burst","outcome":"Yes","size":10.0,"usdcSize":5.0,"transactionHash":"0xtx1","price":0.5,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET, WALLET)
            )

            watch = log_dir / "watch.sh"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=3\\nlatest_new_tx=0xtx3\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" >> %s\nprintf '%s\\n' '---' >> %s\nprintf 'live_submit_status=submitted\\n'\n"
                % ("%s", submit_log, "%s", submit_log),
            )
            self._make_positions_gate_exec(
                positions_gate,
                status="skip_existing_event_position",
                reason="wallet_target_outcome_position_exceeds_latest_trade_size",
                should_follow=False,
                target_size="60.000000",
                other_size="0.000000",
                total_size="60.000000",
                response_count="1",
                event_count="1",
            )
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                ["bash", "scripts/run_rust_follow_last_action_force_live_once.sh", "--user", WALLET],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("poll_new_events=3", completed.stdout)
            forwarded = submit_log.read_text()
            self.assertEqual(forwarded.count("--latest-activity"), 3)
            summary_paths = sorted((state_root / "runs").glob("*/summary.txt"))
            self.assertEqual(len(summary_paths), 3)
            summary_text = "\n".join(path.read_text() for path in summary_paths)
            self.assertIn("latest_tx=0xtx1", summary_text)
            self.assertIn("latest_tx=0xtx2", summary_text)
            self.assertIn("latest_tx=0xtx3", summary_text)
            self.assertIn("follow_trigger_reason=leader_new_open", summary_text)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)

    def test_force_live_once_accepts_external_proxy_argument(self) -> None:
        root = Path.cwd()
        state_root = root / ".omx" / "force-live-follow" / WALLET
        activity_root = root / ".omx" / "live-activity" / WALLET
        log_dir = Path(tempfile.mkdtemp(prefix="force-live-once-proxy-arg-"))
        try:
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
            activity_root.mkdir(parents=True, exist_ok=True)

            latest_activity = activity_root / "latest-activity.json"
            latest_activity.write_text(
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","conditionId":"cond-open","size":60.0,"usdcSize":30.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            watch_args = log_dir / "watch-args.txt"
            positions_gate = log_dir / "positions-gate.sh"
            snapshot = log_dir / "snapshot.sh"
            snapshot_output = log_dir / "dashboard.json"
            submit = log_dir / "submit.sh"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'watch_user=%s\\n'\n"
                % ("%s", watch_args, WALLET),
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf 'live_submit_status=submitted\\n'\n",
            )
            self._make_positions_gate_exec(positions_gate)
            self._make_account_snapshot_exec(
                snapshot,
                '{"account_snapshot":{"positions":[],"open_orders":[]}}',
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
            env["POSITIONS_GATE_BIN_DEFAULT"] = str(positions_gate)
            env["ACCOUNT_SNAPSHOT_BIN_DEFAULT"] = str(snapshot)
            env["ACCOUNT_SNAPSHOT_PATH"] = str(snapshot_output)
            env["LIVE_SUBMIT_BIN_DEFAULT"] = str(submit)
            env["POLYMARKET_CURL_PROXY"] = ""

            completed = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_follow_last_action_force_live_once.sh",
                    "--user",
                    WALLET,
                    "--proxy",
                    "http://proxy.example:8080",
                ],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("proxy=http://proxy.example:8080", completed.stdout)
            forwarded = watch_args.read_text()
            self.assertIn("--proxy", forwarded)
            self.assertIn("http://proxy.example:8080", forwarded)
        finally:
            shutil.rmtree(log_dir, ignore_errors=True)
            shutil.rmtree(state_root, ignore_errors=True)
            shutil.rmtree(activity_root, ignore_errors=True)
