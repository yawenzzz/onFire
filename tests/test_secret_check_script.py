import unittest
from pathlib import Path


class SecretCheckScriptTests(unittest.TestCase):
    def test_secret_check_script_exists(self) -> None:
        text = Path('scripts/check_secrets.sh').read_text()
        self.assertIn('PM_ACCESS_KEY', text)
        self.assertIn('CLOB_API_KEY', text)
