import tempfile
import unittest
from pathlib import Path

from polymarket_arb.auth.local_env_writer import upsert_env_file


class LocalEnvWriterTests(unittest.TestCase):
    def test_updates_existing_keys_and_appends_missing_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / ".env.local"
            path.write_text("PM_ACCESS_KEY=\nCLOB_API_KEY=old-key\n")

            upsert_env_file(
                path,
                {
                    "CLOB_API_KEY": "new-key",
                    "CLOB_SECRET": "secret-value",
                    "CLOB_PASS_PHRASE": "pass-value",
                },
            )

            self.assertEqual(
                path.read_text(),
                "PM_ACCESS_KEY=\n"
                "CLOB_API_KEY=new-key\n"
                "CLOB_SECRET=secret-value\n"
                "CLOB_PASS_PHRASE=pass-value\n",
            )

    def test_preserves_comments_and_unrelated_lines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / ".env.local"
            path.write_text("# keep me\nPM_SIGNATURE=\n\nOTHER=value\n")

            upsert_env_file(path, {"CLOB_SECRET": "secret-value"})

            self.assertEqual(
                path.read_text(),
                "# keep me\nPM_SIGNATURE=\n\nOTHER=value\nCLOB_SECRET=secret-value\n",
            )
