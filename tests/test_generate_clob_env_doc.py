import unittest
from pathlib import Path


class GenerateClobEnvDocTests(unittest.TestCase):
    def test_secret_setup_doc_mentions_generate_script(self) -> None:
        text = Path("docs/secret-setup.md").read_text()
        self.assertIn("scripts/generate_clob_env.sh", text)
        self.assertIn(".env.local", text)
        self.assertIn("PRIVATE_KEY", text)
