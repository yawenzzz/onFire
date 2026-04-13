import unittest

from polymarket_arb.rules.template_whitelist import is_whitelisted


class TemplateWhitelistTests(unittest.TestCase):
    def test_only_safe_templates_are_allowed(self) -> None:
        self.assertTrue(is_whitelisted("exhaustive_set"))
        self.assertTrue(is_whitelisted("directional_ladder"))
        self.assertTrue(is_whitelisted("offset_structure"))
        self.assertFalse(is_whitelisted("semantic_cross_market_ml"))
