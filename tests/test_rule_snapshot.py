import unittest

from polymarket_arb.rules.rule_snapshot import RuleSnapshot


class RuleSnapshotTests(unittest.TestCase):
    def test_same_text_matches_same_hash(self) -> None:
        snapshot = RuleSnapshot.from_text("market resolves to yes if event occurs")
        self.assertTrue(snapshot.matches_text("market resolves to yes if event occurs"))

    def test_changed_text_detects_drift(self) -> None:
        snapshot = RuleSnapshot.from_text("original rules")
        self.assertFalse(snapshot.matches_text("updated rules"))
