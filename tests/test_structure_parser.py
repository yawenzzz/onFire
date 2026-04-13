import unittest

from polymarket_arb.rules.structure_parser import parse_structure


class StructureParserTests(unittest.TestCase):
    def test_parses_directional_ladder(self) -> None:
        result = parse_structure(outcome_count=2, ordered_thresholds=True, offset_relation=False)
        self.assertEqual(result.template_type, "directional_ladder")
        self.assertTrue(result.allowed)

    def test_parses_exhaustive_set(self) -> None:
        result = parse_structure(outcome_count=3, ordered_thresholds=False, offset_relation=False)
        self.assertEqual(result.template_type, "exhaustive_set")
        self.assertTrue(result.allowed)
