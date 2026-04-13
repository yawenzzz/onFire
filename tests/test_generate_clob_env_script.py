import unittest
from pathlib import Path


class GenerateClobEnvScriptTests(unittest.TestCase):
    def test_script_exists_and_mentions_generation_write_and_check_steps(self) -> None:
        text = Path("scripts/generate_clob_env.sh").read_text()
        self.assertIn("derive_clob_creds_python_template.py", text)
        self.assertIn(".env.local", text)
        self.assertIn("check_secrets.sh", text)
        self.assertIn("PRIVATE_KEY", text)
        self.assertIn("CLOB_API_KEY", text)
