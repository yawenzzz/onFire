import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.entrypoint import main


class EntrypointCliTests(unittest.TestCase):
    def test_main_writes_report_and_returns_zero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'report.json'
            code = main([
                '--session-id', 's1',
                '--surface-id', 'polymarket-us',
                '--outcome-count', '2',
                '--ordered-thresholds',
                '--surface-resolved',
                '--jurisdiction-eligible',
                '--output', str(out),
            ])
            self.assertEqual(code, 0)
            report = json.loads(out.read_text())
            self.assertIn('verdict', report)
