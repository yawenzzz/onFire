import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.entrypoint import main


class EntrypointInputFileTests(unittest.TestCase):
    def test_main_can_read_external_input_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            input_path = Path(tmp) / 'input.json'
            output_path = Path(tmp) / 'report.json'
            input_path.write_text(json.dumps({
                'session_id': 's1',
                'surface_id': 'polymarket-us',
                'outcome_count': 2,
                'ordered_thresholds': True,
                'offset_relation': False,
                'legs': [
                    {
                        'market_id': 'm1',
                        'side': 'BUY',
                        'price': 0.4,
                        'market_state': 'OPEN',
                        'tick_valid': True,
                        'visible_depth_qty': 10,
                        'preview_ok': True,
                        'clarification_hash': 'a'
                    }
                ],
                'pi_min_stress_usd': 1.0,
                'hedge_completion_prob': 0.99,
                'capital_efficiency': 0.5,
                'surface_resolved': True,
                'jurisdiction_eligible': True
            }))
            code = main(['--input-file', str(input_path), '--output', str(output_path)])
            self.assertEqual(code, 0)
            report = json.loads(output_path.read_text())
            self.assertIn('verdict', report)
