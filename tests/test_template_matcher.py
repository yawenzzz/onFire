import unittest

from polymarket_arb.rules.template_whitelist import TemplateMatch, match_template


class TemplateMatcherTests(unittest.TestCase):
    def test_matches_exhaustive_set(self) -> None:
        match = match_template(outcome_count=3, ordered_thresholds=False, offset_relation=False)
        self.assertEqual(match.template_type, "exhaustive_set")
        self.assertTrue(match.allowed)

    def test_matches_directional_ladder(self) -> None:
        match = match_template(outcome_count=2, ordered_thresholds=True, offset_relation=False)
        self.assertEqual(match.template_type, "directional_ladder")
        self.assertTrue(match.allowed)

    def test_rejects_unknown_structure(self) -> None:
        match = match_template(outcome_count=1, ordered_thresholds=False, offset_relation=False)
        self.assertEqual(match.template_type, "unsupported")
        self.assertFalse(match.allowed)
