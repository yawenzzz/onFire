import unittest
from pathlib import Path


class PythonCredsTemplateTests(unittest.TestCase):
    def test_python_template_exists_and_mentions_py_clob_client(self) -> None:
        text = Path('scripts/derive_clob_creds_python_template.py').read_text()
        self.assertIn('py_clob_client.client', text)
        self.assertIn('create_or_derive_api_creds', text)
        self.assertIn('SIGNATURE_TYPE', text)
