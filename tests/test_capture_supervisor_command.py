import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_supervisor import run_capture_supervisor_once


class CaptureSupervisorCommandTests(unittest.TestCase):
    def test_capture_supervisor_once_runs_cycle(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            result = run_capture_supervisor_once(root=root, limit=1)
            self.assertEqual(result['captured'], 1)
            self.assertTrue((root / 'dashboard.json').exists())
