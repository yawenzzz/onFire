import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.data.input_schema import load_shadow_input


class InputSchemaLoaderTests(unittest.TestCase):
    def test_loads_shadow_input_and_validates_required_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'input.json'
            path.write_text(json.dumps({
                'session_id': 's1',
                'surface_id': 'polymarket-us',
                'outcome_count': 2,
                'ordered_thresholds': True,
                'offset_relation': False,
                'legs': [],
                'pi_min_stress_usd': 1.0,
                'hedge_completion_prob': 0.99,
                'capital_efficiency': 0.5,
                'surface_resolved': True,
                'jurisdiction_eligible': True,
            }))
            data = load_shadow_input(path)
            self.assertEqual(data['session_id'], 's1')
            self.assertEqual(data['surface_id'], 'polymarket-us')

    def test_raises_on_missing_required_key(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'input.json'
            path.write_text(json.dumps({'session_id': 's1'}))
            with self.assertRaises(ValueError):
                load_shadow_input(path)
