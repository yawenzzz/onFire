import unittest
from pathlib import Path


class GitignoreSecretsTests(unittest.TestCase):
    def test_gitignore_protects_local_env_files(self) -> None:
        text = Path('.gitignore').read_text()
        self.assertIn('.env.local', text)
        self.assertIn('.env.*.local', text)
