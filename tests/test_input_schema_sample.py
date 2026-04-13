import json
import unittest
from pathlib import Path


class InputSchemaSampleTests(unittest.TestCase):
    def test_sample_input_schema_exists_with_required_fields(self) -> None:
        path = Path('examples/shadow-input.json')
        self.assertTrue(path.exists())
        data = json.loads(path.read_text())
        for key in [
            'session_id',
            'surface_id',
            'outcome_count',
            'ordered_thresholds',
            'offset_relation',
            'legs',
            'pi_min_stress_usd',
            'hedge_completion_prob',
            'capital_efficiency',
            'surface_resolved',
            'jurisdiction_eligible',
        ]:
            self.assertIn(key, data)
