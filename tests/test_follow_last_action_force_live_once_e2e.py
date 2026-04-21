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
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % wallet
            )

            watch = log_dir / "watch.sh"
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-args.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\n'\n" % wallet,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > %s\nprintf 'live_submit_status=submitted\\n'\nprintf 'leader_price=0.50000000\\n'\nprintf 'follower_effective_price=0.50000000\\n'\nprintf 'price_gap_bps=0.0000\\n'\n"
                % ("%s", submit_log),
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
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
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=1\\nlatest_new_tx=0xdup\\n'\n"
                % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf 'called\\n' >> %s\nprintf 'live_submit_status=submitted\\n'\n"
                % submit_log,
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
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
            submit = log_dir / "submit.sh"
            submit_log = log_dir / "submit-called.txt"

            self._make_exec(
                watch,
                "#!/usr/bin/env bash\nprintf 'watch_user=%s\\npoll_new_events=0\\n'\n" % WALLET,
            )
            self._make_exec(
                submit,
                "#!/usr/bin/env bash\nprintf 'called\\n' >> %s\nprintf 'live_submit_status=submitted\\n'\n"
                % submit_log,
            )

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
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
                '[{"proxyWallet":"%s","timestamp":10,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xold","price":0.5,"side":"BUY","slug":"market-a"},{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % (WALLET, WALLET)
            )

            watch = log_dir / "watch.sh"
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

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
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
                '[{"proxyWallet":"%s","timestamp":20,"type":"TRADE","asset":"asset-1","size":2.0,"usdcSize":1.0,"transactionHash":"0xnew","price":0.5,"side":"BUY","slug":"market-a"}]'
                % WALLET
            )

            watch = log_dir / "watch.sh"
            watch_args = log_dir / "watch-args.txt"
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

            env = os.environ.copy()
            env["WATCH_BIN_DEFAULT"] = str(watch)
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
