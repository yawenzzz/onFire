import os
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.auth.local_env_loader import load_dotenv_file


class LocalEnvLoaderTests(unittest.TestCase):
    def test_loads_key_value_pairs_into_env(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / '.env.local'
            path.write_text('PM_ACCESS_KEY=abc\nCLOB_SECRET=def\n')
            load_dotenv_file(path)
            self.assertEqual(os.environ.get('PM_ACCESS_KEY'), 'abc')
            self.assertEqual(os.environ.get('CLOB_SECRET'), 'def')
