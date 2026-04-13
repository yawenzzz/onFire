import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.supervisor_cli import main


class SupervisorCliTests(unittest.TestCase):
    def test_supervisor_cli_runs_once_and_writes_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code = main(['--root', str(root), '--limit', '1'])
            self.assertEqual(code, 0)
            self.assertTrue((root / 'capture.jsonl').exists())
            self.assertTrue((root / 'metrics.json').exists())
            self.assertTrue((root / 'health.json').exists())
            self.assertTrue((root / 'alerts.json').exists())
