import unittest

from polymarket_arb.data.jsonl_schema import validate_capture_record


class JsonlSchemaTests(unittest.TestCase):
    def test_valid_record_passes(self) -> None:
        record = {'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}
        self.assertEqual(validate_capture_record(record), [])

    def test_missing_fields_are_reported(self) -> None:
        errors = validate_capture_record({'market_id': 'm1'})
        self.assertIn('missing field: market_state', errors)
