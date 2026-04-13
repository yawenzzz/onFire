import unittest
from pathlib import Path


class MakefileTests(unittest.TestCase):
    def test_makefile_exists_with_core_targets(self) -> None:
        path = Path('Makefile')
        self.assertTrue(path.exists())
        text = path.read_text()
        for target in ['test:', 'shadow-demo:', 'lint:']:
            self.assertIn(target, text)
