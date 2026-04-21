import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


WALLET = "0x11084005d88A0840b5F38F8731CCa9152BbD99F7"


class MinmaxFollowLiveSubmitE2ETests(unittest.TestCase):
    def _make_exec(self, path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_live_submit_wrapper_retries_force_follow_once_until_success(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="minmax-live-submit-e2e-") as tmpdir:
            tmp = Path(tmpdir)
            force_once = tmp / "force_once.sh"
            state = tmp / "state.txt"
            self._make_exec(
                force_once,
                f"""#!/usr/bin/env bash
state="{state}"
count=0
if [[ -f "$state" ]]; then count=$(cat "$state"); fi
count=$((count+1))
printf '%s' "$count" > "$state"
printf 'force-once-%s\\n' "$count"
printf 'require-new-activity=%s\\n' "${{REQUIRE_NEW_ACTIVITY:-}}"
if [[ "$count" -ge 2 ]]; then
  exit 0
fi
exit 1
""",
            )

            completed = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_minmax_follow_live_submit.sh",
                    "--user",
                    WALLET,
                ],
                cwd=root,
                env={
                    **os.environ,
                    "FORCE_FOLLOW_ONCE_BIN": str(force_once),
                    "FOLLOW_FOREVER": "0",
                    "RESTART_ON_FAILURE": "1",
                    "MAX_RESTARTS": "5",
                    "RESTART_DELAY_SECONDS": "0",
                },
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("rust live submit continuous follow", completed.stdout)
            self.assertIn(f"force_follow_once_bin={force_once}", completed.stdout)
            self.assertIn("force-once-1", completed.stdout)
            self.assertIn("force-once-2", completed.stdout)
            self.assertIn("require-new-activity=1", completed.stdout)
            self.assertEqual(state.read_text(), "2")

    def test_live_submit_wrapper_exits_cleanly_when_force_follow_once_skips_no_new_activity(self) -> None:
        root = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="minmax-live-submit-skip-e2e-") as tmpdir:
            tmp = Path(tmpdir)
            force_once = tmp / "force_once.sh"
            state = tmp / "state.txt"
            self._make_exec(
                force_once,
                f"""#!/usr/bin/env bash
state="{state}"
printf '1' > "$state"
printf 'require-new-activity=%s\\n' "${{REQUIRE_NEW_ACTIVITY:-}}"
printf 'skipping because watch reported no new activity\\n'
exit 0
""",
            )

            completed = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_minmax_follow_live_submit.sh",
                    "--user",
                    WALLET,
                ],
                cwd=root,
                env={
                    **os.environ,
                    "FORCE_FOLLOW_ONCE_BIN": str(force_once),
                    "FOLLOW_FOREVER": "0",
                    "RESTART_ON_FAILURE": "0",
                },
                capture_output=True,
                text=True,
                check=True,
            )

            self.assertIn("rust live submit continuous follow", completed.stdout)
            self.assertIn(f"force_follow_once_bin={force_once}", completed.stdout)
            self.assertIn("require-new-activity=1", completed.stdout)
            self.assertIn("skipping because watch reported no new activity", completed.stdout)
            self.assertEqual(state.read_text(), "1")
