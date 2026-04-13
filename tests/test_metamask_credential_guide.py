import unittest
from pathlib import Path


class MetaMaskCredentialGuideTests(unittest.TestCase):
    def test_guide_exists_and_mentions_clob_creds(self) -> None:
        text = Path('docs/metamask-credential-guide.md').read_text()
        self.assertIn('CLOB_API_KEY', text)
        self.assertIn('CLOB_SECRET', text)
        self.assertIn('CLOB_PASS_PHRASE', text)
        self.assertIn('MetaMask', text)

    def test_template_script_exists(self) -> None:
        text = Path('scripts/derive_clob_creds_template.sh').read_text()
        self.assertIn('template script only', text)
        self.assertIn('CLOB_API_KEY', text)
