import unittest
from pathlib import Path


class CredentialAcquisitionDocTests(unittest.TestCase):
    def test_doc_exists_with_expected_sections(self) -> None:
        text = Path('docs/credential-acquisition.md').read_text()
        for marker in [
            'CLOB credentials',
            'WebSocket auth fields',
            'Safe local setup',
            'Verification',
            'Current boundary',
        ]:
            self.assertIn(marker, text)

    def test_doc_mentions_supported_env_names(self) -> None:
        text = Path('docs/credential-acquisition.md').read_text()
        for name in [
            'CLOB_API_KEY',
            'CLOB_SECRET',
            'CLOB_PASS_PHRASE',
            'POLYMARKET_KEY_ID',
            'POLYMARKET_SECRET_KEY',
            'PM_ACCESS_KEY',
            'PM_SIGNATURE',
            'PM_TIMESTAMP',
            'scripts/generate_pm_auth.sh',
        ]:
            self.assertIn(name, text)
