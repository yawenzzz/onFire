import base64
import os
import subprocess
import unittest


def _private_key_base64() -> str:
    seed = bytes(range(32))
    public = bytes(reversed(range(32)))
    return base64.b64encode(seed + public).decode("utf-8")


class GeneratePMAuthScriptTests(unittest.TestCase):
    def test_script_exports_pm_values_when_sourced(self) -> None:
        env = dict(os.environ)
        env["POLYMARKET_KEY_ID"] = "key-123"
        env["POLYMARKET_SECRET_KEY"] = _private_key_base64()
        env["PM_PATH"] = "/v1/ws/markets"
        env["PM_TIMESTAMP_OVERRIDE"] = "1705420800000"

        completed = subprocess.run(
            [
                "bash",
                "-lc",
                "source scripts/generate_pm_auth.sh >/dev/null && "
                "printf '%s\\n%s\\n%s\\n' \"$PM_ACCESS_KEY\" \"$PM_TIMESTAMP\" \"$PM_SIGNATURE\"",
            ],
            cwd="/Users/yawen.zheng/onFire",
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )

        lines = completed.stdout.strip().splitlines()
        self.assertEqual(lines[0], "key-123")
        self.assertEqual(lines[1], "1705420800000")
        self.assertTrue(lines[2])
